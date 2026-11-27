use starlark_syntax::eval_exception::EvalException;

use super::Bc;
use super::dispatch;
use crate::eval::Evaluator;
use crate::eval::ExecutionBudget;
use crate::eval::bc::addr::BcPtrAddr;
use crate::eval::bc::frame::BcFramePtr;
use crate::eval::bc::instr::InstrControl;
use crate::eval::bc::instr_impl::InstrBreak;
use crate::eval::bc::instr_impl::InstrContinue;
use crate::eval::bc::instr_impl::InstrIter;
use crate::eval::bc::instr_impl::InstrIterStop;
use crate::eval::bc::opcode::BcOpcode;
use crate::eval::bc::stack_ptr::BcSlotIn;
use crate::eval::runtime::evaluator::EvaluationCallbacks;
use crate::values::Value;

#[cfg(test)]
mod tests;

/// Slots are reserved while their iterator is live. A return expression can
/// open another iteration after stopping its outer loops.
struct ActiveLoops<'v> {
    frame: BcFramePtr<'v>,
    slots: Vec<BcSlotIn>,
}

impl ActiveLoops<'_> {
    fn advance(&mut self, ip: BcPtrAddr<'_>, next: BcPtrAddr<'_>, opcode: BcOpcode) {
        match opcode {
            BcOpcode::Iter => {
                if next == ip.add_instr::<InstrIter>() {
                    let (_, _, slot, _, _) = ip.get_instr::<InstrIter>().arg;
                    assert!(self.slots.len() < self.slots.capacity());
                    self.slots.push(slot.get().to_in());
                }
            }
            BcOpcode::Continue => {
                let (slot, _, _, _, end) = ip.get_instr::<InstrContinue>().arg;
                if next == ip.add_rel(end) {
                    self.closed(slot);
                }
            }
            BcOpcode::Break => self.closed(ip.get_instr::<InstrBreak>().arg.0),
            BcOpcode::IterStop => self.closed(ip.get_instr::<InstrIterStop>().arg),
            _ => {}
        }
    }

    fn closed(&mut self, slot: BcSlotIn) {
        let active = self.slots.pop();
        debug_assert_eq!(active, Some(slot));
    }

    fn stop(&mut self) {
        while let Some(slot) = self.slots.pop() {
            self.frame.get_bc_slot(slot).get_ref().iter_stop();
        }
    }
}

impl Drop for ActiveLoops<'_> {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(super) fn run_block<'v, EC: EvaluationCallbacks>(
    bc: &Bc,
    eval: &mut Evaluator<'v, '_, '_>,
    ec: &mut EC,
    budget: &ExecutionBudget,
) -> Result<Value<'v>, EvalException> {
    let frame = eval.current_frame;
    let mut loops = ActiveLoops {
        frame,
        slots: Vec::new(),
    };
    budget
        .reserve_vec(&mut loops.slots, bc.max_loop_depth.0 as usize)
        .map_err(|limit| EvalException::new_unknown_span(limit.into()))?;
    let mut ip = bc.instrs.start_ptr();
    loop {
        budget
            .charge(1, 0)
            .map_err(|limit| EvalException::new_unknown_span(limit.into()))?;
        let opcode = ip.get_opcode();
        let admission = ec.before_instr(eval, ip, opcode).and_then(|()| {
            budget.check()?;
            if opcode == BcOpcode::Iter {
                // Hooks can edit locals. Inspect the actual operand after the
                // hook and pay for unwind before the iterator can acquire a lock.
                let (slot, _, _, _, _) = ip.get_instr::<InstrIter>().arg;
                budget.charge(1, 0)?;
                eval.heap()
                    .admit_bounded_iteration(frame.get_bc_slot(slot), "bytecode iteration")?;
            }
            Ok(())
        });
        let control = match admission {
            Ok(()) => dispatch(eval, frame, ip, opcode),
            Err(error) => InstrControl::Err(error),
        };
        match control {
            InstrControl::Next(next) => {
                loops.advance(ip, next, opcode);
                ip = next;
            }
            InstrControl::Return(value) => {
                debug_assert!(loops.slots.is_empty());
                budget
                    .check()
                    .map_err(|limit| EvalException::new_unknown_span(limit.into()))?;
                return Ok(value);
            }
            InstrControl::Err(error) => {
                loops.stop();
                return Err(match budget.check() {
                    Ok(()) => Bc::wrap_error_for_instr_ptr(ip, error, eval),
                    Err(limit) => EvalException::new_unknown_span(limit.into()),
                });
            }
        }
    }
}
