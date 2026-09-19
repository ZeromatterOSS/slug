//! Only DICE-attested transient root completions may relax dirty-root selection.
use async_trait::async_trait;
use dice::CancellationContext;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::Key;
use dice::UserComputationData;
use slug_workspace_v2::NormalizedAbsolutePath;
use tokio::sync::Notify;

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Allocative)]
struct Mode;
impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("transient-root-mode")
    }
}
impl InjectedKey for Mode {
    type Value = u8;
    fn equality(a: &u8, b: &u8) -> bool {
        a == b
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Allocative)]
enum Graph {
    Leaf,
    Parent,
    OtherParent,
}
impl fmt::Display for Graph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "transient-root-{self:?}")
    }
}
struct Gate {
    entered: Notify,
    release: Notify,
}
#[async_trait]
impl Key for Graph {
    type Value = u8;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> u8 {
        let value = match self {
            Self::Leaf => ctx.compute(&Mode).await.unwrap(),
            Self::Parent | Self::OtherParent => {
                if *self == Self::OtherParent {
                    if let Ok(gate) = ctx.per_transaction_data().data.get::<Arc<Gate>>() {
                        gate.entered.notify_one();
                        gate.release.notified().await;
                    }
                }
                // Parent values themselves remain valid; only the Leaf makes
                // their completed computation transient through dependencies.
                10 + ctx.compute(&Self::Leaf).await.unwrap()
            }
        };
        ctx.store_evaluation_data(EventBatch::from_events([EvaluationEvent::StarlarkPrint {
            location: slug_events_v2::StarlarkSourceLocation::new(Arc::from("transient.bzl"), 1, 1),
            text: format!("{self:?}").into(),
        }]))
        .unwrap();
        value
    }
    fn equality(a: &u8, b: &u8) -> bool {
        a == b
    }
    fn validity(value: &u8) -> bool {
        *value != 1
    }
}

async fn attempt(
    dice: &Arc<Dice>,
    owner: &Arc<CommandEffectOwner>,
    mode: u8,
    gate: Option<Arc<Gate>>,
) -> anyhow::Result<(Arc<AttemptEffectTracker>, DiceTransaction)> {
    let tracker = owner.begin_attempt()?;
    let mut data = UserComputationData::default();
    WorkspaceDemandOwner::new(dice, NormalizedAbsolutePath::new("/workspace").unwrap()).install(
        dice,
        &mut data,
        Some(tracker.clone()),
    )?;
    if let Some(gate) = gate {
        data.data.set(gate);
    }
    let mut updater = dice.updater_with_data(data);
    updater.changed_to([(Mode, mode)])?;
    Ok((tracker, updater.commit().await))
}

async fn prime(dice: &Arc<Dice>, owner: &Arc<CommandEffectOwner>) -> anyhow::Result<()> {
    let (tracker, mut transaction) = attempt(dice, owner, 0, None).await?;
    transaction.compute(&Graph::Parent).await?;
    transaction.compute(&Graph::OtherParent).await?;
    tracker
        .seal_terminal()?
        .select(&transaction)
        .await?
        .into_parts()
        .2
        .reset_to_idle()?;
    Ok(())
}

#[tokio::test]
async fn current_transient_completion_can_omit_dirty_root_and_recover() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let owner = CommandEffectOwner::new();
    for mode in [0, 1, 0] {
        let (tracker, mut transaction) = attempt(&dice, &owner, mode, None).await?;
        assert_eq!(transaction.compute(&Graph::Parent).await?, mode + 10);
        let sealed = tracker.seal_terminal_allowing_unavailable_roots()?;
        assert_eq!(sealed.roots.len(), 1);
        if mode == 1 {
            assert_eq!(sealed.transient_roots.as_slice(), sealed.roots.as_ref());
            assert_eq!(
                transaction
                    .activation_closure(sealed.roots.iter().copied())
                    .await
                    .unwrap_err(),
                ActivationClosureError::Dirty {
                    node: sealed.roots[0],
                    version: transaction.version(),
                }
            );
        } else {
            assert!(sealed.transient_roots.is_empty());
        }
        let selected = sealed.select(&transaction).await?;
        let (events, _, terminal) = selected.into_parts();
        let batches = events.reconcile(&AcceptedEventEpoch::empty()).0;
        let text = batches
            .batches()
            .iter()
            .flat_map(EventBatch::events)
            .map(|event| match event {
                EvaluationEvent::StarlarkPrint { text, .. } => text.as_str(),
                _ => panic!("unexpected diagnostic"),
            })
            .collect::<Vec<_>>();
        if mode == 1 {
            assert!(text.is_empty());
        } else {
            assert_eq!(text, ["Leaf", "Parent"]);
        }
        terminal.reset_to_idle()?;
    }
    Ok(())
}

#[tokio::test]
async fn all_transient_roots_still_check_strict_foreign_stale_and_unverified() -> anyhow::Result<()>
{
    for case in ["strict", "foreign", "stale", "unverified"] {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let owner = CommandEffectOwner::new();
        prime(&dice, &owner).await?;
        let (tracker, mut transaction) = attempt(&dice, &owner, 1, None).await?;
        transaction.compute(&Graph::Parent).await?;
        let sealed = if case == "strict" {
            tracker.seal_terminal()?
        } else {
            tracker.seal_terminal_allowing_unavailable_roots()?
        };
        assert_eq!(sealed.transient_roots.as_slice(), sealed.roots.as_ref());
        let root = sealed.roots[0];
        let error = match case {
            "strict" => sealed.select(&transaction).await.unwrap_err(),
            "foreign" => {
                let foreign = Dice::builder().build(DetectCycles::Enabled);
                let foreign_transaction = foreign.updater().commit().await;
                sealed.select(&foreign_transaction).await.unwrap_err()
            }
            "stale" => {
                let mut updater = dice.updater();
                updater.changed_to([(Mode, 2)])?;
                let newer = updater.commit().await;
                assert_ne!(newer.version(), transaction.version());
                sealed.select(&newer).await.unwrap_err()
            }
            "unverified" => {
                let _cleared = transaction
                    .dupe()
                    .into_updater()
                    .unstable_take()
                    .commit()
                    .await;
                sealed.select(&transaction).await.unwrap_err()
            }
            _ => unreachable!(),
        };
        match (case, error) {
            (
                "strict" | "stale",
                CommandEffectError::Closure(ActivationClosureError::Dirty { node, .. }),
            ) => assert_eq!(node, root),
            (
                "foreign",
                CommandEffectError::Closure(ActivationClosureError::ForeignEngine { node }),
            ) => assert_eq!(node, root),
            (
                "unverified",
                CommandEffectError::Closure(ActivationClosureError::NotVerified { node, .. }),
            ) => assert_eq!(node, root),
            (_, error) => panic!("{case} wrong rejection: {error:?}"),
        }
    }
    Ok(())
}

#[tokio::test]
async fn allowed_transient_root_does_not_hide_an_uncompleted_dirty_root() -> anyhow::Result<()> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let owner = CommandEffectOwner::new();
    prime(&dice, &owner).await?;
    let gate = Arc::new(Gate {
        entered: Notify::new(),
        release: Notify::new(),
    });
    let (tracker, mut transaction) = attempt(&dice, &owner, 1, Some(gate.clone())).await?;
    transaction.compute(&Graph::Parent).await?;
    {
        let request = transaction.compute(&Graph::OtherParent);
        tokio::pin!(request);
        tokio::select! {
            result = &mut request => panic!("gated root unexpectedly completed: {result:?}"),
            _ = gate.entered.notified() => {}
        }
        // Dropping the caller before successful completion supplies no attestation.
    }
    let sealed = tracker.seal_terminal_allowing_unavailable_roots()?;
    assert_eq!(sealed.roots.len(), 2);
    assert_eq!(sealed.transient_roots.as_slice(), &sealed.roots[..1]);
    let uncompleted = sealed.roots[1];
    let error = sealed.select(&transaction).await.unwrap_err();
    assert_eq!(
        error,
        CommandEffectError::Closure(ActivationClosureError::Dirty {
            node: uncompleted,
            version: transaction.version()
        })
    );
    gate.release.notify_one();
    Ok(())
}
