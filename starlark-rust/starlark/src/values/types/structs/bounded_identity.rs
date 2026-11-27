use std::hash::Hash;
use std::hash::Hasher;

use starlark_map::StarlarkHashValue;
use starlark_map::StarlarkHasher;

use crate::eval::ExecutionBudget;
use crate::eval::ExecutionLimit;
use crate::eval::is_starlark_def;
use crate::values::Value;
use crate::values::ValueLike;
use crate::values::dict::DictRef;
use crate::values::dict::value::DictGen;
use crate::values::dict::value::FrozenHashableDictData;
use crate::values::dict::value::HashableDictData;
use crate::values::error::ControlError;
use crate::values::float::StarlarkFloat;
use crate::values::list::ListRef;
use crate::values::structs::unordered_hasher::UnorderedHasher;
use crate::values::structs::value::Struct;
use crate::values::tuple::TupleRef;

// Bound host stack usage even when a caller supplies a larger depth limit.
const MAX_DEPTH: usize = 64;

/// An identity operation requiring native admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructuralIdentityOp {
    /// Check whether the value can be nested in a hashable container.
    Immutability,
    /// Hash the value.
    Hash,
    /// Compare the value for equality.
    Equality,
}

impl StructuralIdentityOp {
    fn name(self) -> &'static str {
        match self {
            Self::Immutability => "structural immutability",
            Self::Hash => "structural hashing",
            Self::Equality => "structural equality",
        }
    }
}

/// A closed native extension to bounded structural identity.
pub trait StructuralIdentityNative<'v>: Sized {
    /// Admit the exact operation on a native atom before ordinary value dispatch.
    /// An admitted atom is immutable and its identity operations never recurse
    /// through arbitrary Starlark values. Return false for an unrecognized type.
    fn admit_atomic(
        &self,
        value: Value<'v>,
        op: StructuralIdentityOp,
        budget: &ExecutionBudget,
    ) -> Result<bool, ExecutionLimit>;

    /// Check a recognized native composite using this context for its children.
    /// Return None for an unrecognized type.
    fn is_immutable(
        &self,
        ctx: &mut StructuralIdentity<'_, 'v, Self>,
        value: Value<'v>,
    ) -> crate::Result<Option<bool>>;

    /// Write the recognized composite's ordinary hash, using structural child
    /// framing when requested. Return false for an unrecognized type.
    fn write_hash(
        &self,
        ctx: &mut StructuralIdentity<'_, 'v, Self>,
        value: Value<'v>,
        hasher: &mut StarlarkHasher,
        structural: bool,
    ) -> crate::Result<bool>;

    /// Compare a recognized native left value, using this context for children.
    /// Return None for an unrecognized left type.
    fn equals(
        &self,
        ctx: &mut StructuralIdentity<'_, 'v, Self>,
        left: Value<'v>,
        right: Value<'v>,
    ) -> crate::Result<Option<bool>>;
}

#[derive(Clone, Copy)]
struct Visit<'v> {
    op: StructuralIdentityOp,
    left: Value<'v>,
    right: Value<'v>,
}

impl Visit<'_> {
    fn repeats(self, other: Self) -> bool {
        if self.op != other.op {
            return false;
        }
        if self.op == StructuralIdentityOp::Equality {
            (self.left.ptr_eq(other.left) && self.right.ptr_eq(other.right))
                || (self.left.ptr_eq(other.right) && self.right.ptr_eq(other.left))
        } else {
            self.left.ptr_eq(other.left)
        }
    }
}

/// A request-scoped structural walker with no implicit native dispatch.
pub struct StructuralIdentity<'a, 'v, N> {
    budget: &'a ExecutionBudget,
    native: &'a N,
    depth: usize,
    path: [Visit<'v>; MAX_DEPTH],
}

impl<'a, 'v, N: StructuralIdentityNative<'v>> StructuralIdentity<'a, 'v, N> {
    /// Build an identity walker for the admitted types of this request.
    pub fn new(budget: &'a ExecutionBudget, native: &'a N) -> Self {
        Self {
            budget,
            native,
            depth: 0,
            path: [Visit {
                op: StructuralIdentityOp::Immutability,
                left: Value::new_none(),
                right: Value::new_none(),
            }; MAX_DEPTH],
        }
    }

    /// The shared request budget for native field and scratch operations.
    pub fn budget(&self) -> &'a ExecutionBudget {
        self.budget
    }

    /// Check the existing immutable-container rules with admitted work.
    pub fn is_immutable(&mut self, value: Value<'v>) -> crate::Result<bool> {
        self.visit(
            StructuralIdentityOp::Immutability,
            value,
            Value::new_none(),
            |ctx| ctx.immutable(value),
        )
    }

    /// Write an admitted value through the ordinary or structural hash path.
    pub fn write_hash(
        &mut self,
        value: Value<'v>,
        hasher: &mut StarlarkHasher,
        structural: bool,
    ) -> crate::Result<()> {
        self.visit(
            StructuralIdentityOp::Hash,
            value,
            Value::new_none(),
            |ctx| {
                if ctx.admit_atomic(value, StructuralIdentityOp::Hash)? {
                    return value.write_hash(hasher);
                }
                ctx.composite_hash(value, hasher, structural)
            },
        )
    }

    /// Return the same small hash used by ordinary Starlark value lookup.
    pub fn leaf_hash(&mut self, value: Value<'v>) -> crate::Result<u64> {
        self.small_hash(value).map(|hash| u64::from(hash.get()))
    }

    pub(crate) fn small_hash(&mut self, value: Value<'v>) -> crate::Result<StarlarkHashValue> {
        self.visit(
            StructuralIdentityOp::Hash,
            value,
            Value::new_none(),
            |ctx| {
                if ctx.admit_atomic(value, StructuralIdentityOp::Hash)? {
                    return Ok(value.get_hashed()?.hash());
                }
                let mut hasher = StarlarkHasher::new();
                ctx.composite_hash(value, &mut hasher, false)?;
                Ok(hasher.finish_small())
            },
        )
    }

    /// Compare two admitted values with ordinary equality and fallible key lookup.
    pub fn equals(&mut self, left: Value<'v>, right: Value<'v>) -> crate::Result<bool> {
        if left.ptr_eq(right) {
            self.budget.charge(1, 0)?;
            return Ok(true);
        }
        self.visit(StructuralIdentityOp::Equality, left, right, |ctx| {
            ctx.equal_values(left, right)
        })
    }

    fn visit<T>(
        &mut self,
        op: StructuralIdentityOp,
        left: Value<'v>,
        right: Value<'v>,
        operation: impl FnOnce(&mut Self) -> crate::Result<T>,
    ) -> crate::Result<T> {
        self.budget.charge(1 + self.depth, 0)?;
        if self.depth >= self.budget.text_depth().min(MAX_DEPTH) {
            return Err(self.budget.refuse(ExecutionLimit::StructuralDepth).into());
        }
        let entry = Visit { op, left, right };
        if self.path[..self.depth]
            .iter()
            .any(|prior| entry.repeats(*prior))
        {
            return Err(self.budget.refuse(ExecutionLimit::StructuralCycle).into());
        }
        self.path[self.depth] = entry;
        self.depth += 1;
        let result = operation(self);
        self.depth -= 1;
        self.budget.check()?;
        result
    }

    fn unsupported(&self, value: Value<'v>, op: StructuralIdentityOp) -> crate::Error {
        self.budget
            .refuse(ExecutionLimit::UnsupportedOperation {
                operation: op.name(),
                value_type: value.get_type(),
            })
            .into()
    }

    fn admit_atomic(&self, value: Value<'v>, op: StructuralIdentityOp) -> crate::Result<bool> {
        if let Some(text) = value.unpack_str() {
            if op != StructuralIdentityOp::Immutability {
                self.budget.charge_text_work(text.len())?;
            }
            return Ok(true);
        }
        if value.is_none()
            || value.unpack_bool().is_some()
            || value.unpack_i32().is_some()
            || value.downcast_ref::<StarlarkFloat>().is_some()
        {
            return Ok(true);
        }
        Ok(self.native.admit_atomic(value, op, self.budget)?)
    }

    fn immutable(&mut self, value: Value<'v>) -> crate::Result<bool> {
        if is_starlark_def(value) {
            return Ok(true);
        }
        if let Some(record) = Struct::from_value(value) {
            return self.all_immutable(record.fields.values().copied());
        }
        if let Some(list) = ListRef::from_value(value) {
            if value.unpack_frozen().is_some() {
                return Ok(true);
            }
            return if ListRef::is_immutable(value) {
                self.all_immutable(list.iter())
            } else {
                Ok(false)
            };
        }
        if let Some(dict) = DictRef::from_value(value) {
            if value.unpack_frozen().is_some() {
                return Ok(true);
            }
            return if DictRef::is_immutable(value) {
                self.all_immutable(dict.iter().flat_map(|(key, value)| [key, value]))
            } else {
                Ok(false)
            };
        }
        if let Some(tuple) = TupleRef::from_value(value) {
            return self.all_immutable(tuple.iter());
        }
        let native = self.native;
        if let Some(immutable) = native.is_immutable(self, value)? {
            return Ok(immutable);
        }
        if self.admit_atomic(value, StructuralIdentityOp::Immutability)? {
            return Ok(true);
        }
        Err(self.unsupported(value, StructuralIdentityOp::Immutability))
    }

    fn all_immutable(&mut self, values: impl Iterator<Item = Value<'v>>) -> crate::Result<bool> {
        for value in values {
            if !self.is_immutable(value)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn composite_hash(
        &mut self,
        value: Value<'v>,
        hasher: &mut StarlarkHasher,
        structural: bool,
    ) -> crate::Result<()> {
        if let Some(record) = Struct::from_value(value) {
            if !structural && !self.all_immutable(record.fields.values().copied())? {
                return Err(crate::Error::new_other(anyhow::anyhow!(
                    "unhashable type: struct"
                )));
            }
            let mut unordered = UnorderedHasher::new();
            for (key, value) in record.fields.iter_hashed() {
                let mut entry = StarlarkHasher::new();
                key.hash().hash(&mut entry);
                self.write_hash(*value, &mut entry, true)?;
                unordered.write_hash(entry.finish());
            }
            hasher.write_u64(unordered.finish());
            return Ok(());
        }
        if let Some(list) = ListRef::from_value(value) {
            if !structural || !ListRef::is_immutable(value) {
                return Err(unhashable(value));
            }
            "list".hash(hasher);
            list.len().hash(hasher);
            return self.hash_values(list.iter(), hasher, true);
        }
        if let Some(dict) = DictRef::from_value(value) {
            let hashable = if structural {
                DictRef::is_immutable(value)
            } else {
                value
                    .downcast_ref::<DictGen<HashableDictData<'v>>>()
                    .is_some()
                    || value
                        .downcast_ref::<DictGen<FrozenHashableDictData>>()
                        .is_some()
            };
            if !hashable {
                return Err(unhashable(value));
            }
            return self.hash_dict(&dict, hasher, structural);
        }
        if let Some(tuple) = TupleRef::from_value(value) {
            if structural {
                "tuple".hash(hasher);
                tuple.len().hash(hasher);
            }
            return self.hash_values(tuple.iter(), hasher, structural);
        }
        let native = self.native;
        if native.write_hash(self, value, hasher, structural)? {
            return Ok(());
        }
        Err(self.unsupported(value, StructuralIdentityOp::Hash))
    }

    fn hash_values(
        &mut self,
        values: impl Iterator<Item = Value<'v>>,
        hasher: &mut StarlarkHasher,
        structural: bool,
    ) -> crate::Result<()> {
        for value in values {
            self.write_hash(value, hasher, structural)?;
        }
        Ok(())
    }

    fn hash_dict(
        &mut self,
        dict: &DictRef<'v>,
        hasher: &mut StarlarkHasher,
        structural: bool,
    ) -> crate::Result<()> {
        "dict".hash(hasher);
        dict.len().hash(hasher);
        let mut entries = Vec::new();
        self.budget.reserve_vec(&mut entries, dict.len())?;
        for (key, value) in dict.iter() {
            self.budget.charge(1, 0)?;
            let mut entry = StarlarkHasher::new();
            self.write_hash(key, &mut entry, structural)?;
            self.write_hash(value, &mut entry, structural)?;
            entries.push(entry.finish());
        }
        self.budget
            .sort_unstable_by(&mut entries, |left, right| Ok(left.cmp(right)))?;
        self.budget.charge(entries.len(), 0)?;
        for entry in entries {
            entry.hash(hasher);
        }
        Ok(())
    }

    fn equal_values(&mut self, left: Value<'v>, right: Value<'v>) -> crate::Result<bool> {
        if let Some(left) = Struct::from_value(left) {
            let Some(right) = Struct::from_value(right) else {
                return Ok(false);
            };
            if left.fields.len() != right.fields.len() {
                return Ok(false);
            }
            for (key, value) in left.fields.iter_hashed() {
                let mut matched = None;
                for (candidate, other) in right.fields.iter_hashed() {
                    self.budget.charge(1, 0)?;
                    if key.hash() != candidate.hash() {
                        continue;
                    }
                    let key = key.key().as_str();
                    self.budget.charge_text_work(key.len())?;
                    if key == candidate.key().as_str() {
                        matched = Some(*other);
                        break;
                    }
                }
                match matched {
                    Some(other) if self.equals(*value, other)? => {}
                    _ => return Ok(false),
                }
            }
            return Ok(true);
        }
        if let Some(left) = ListRef::from_value(left) {
            let Some(right) = ListRef::from_value(right) else {
                return Ok(false);
            };
            return self.equal_sequence(left.content(), right.content());
        }
        if let Some(left) = DictRef::from_value(left) {
            let Some(right) = DictRef::from_value(right) else {
                return Ok(false);
            };
            if left.len() != right.len() {
                return Ok(false);
            }
            for (key, value) in left.iter_hashed() {
                let mut matched = None;
                for (candidate, other) in right.iter_hashed() {
                    self.budget.charge(1, 0)?;
                    if key.hash() == candidate.hash()
                        && self.equals(*key.key(), *candidate.key())?
                    {
                        matched = Some(other);
                        break;
                    }
                }
                match matched {
                    Some(other) if self.equals(value, other)? => {}
                    _ => return Ok(false),
                }
            }
            return Ok(true);
        }
        if let Some(left) = TupleRef::from_value(left) {
            let Some(right) = TupleRef::from_value(right) else {
                return Ok(false);
            };
            return self.equal_sequence(left.content(), right.content());
        }
        let native = self.native;
        if let Some(equal) = native.equals(self, left, right)? {
            return Ok(equal);
        }
        if !self.admit_atomic(left, StructuralIdentityOp::Equality)? {
            return Err(self.unsupported(left, StructuralIdentityOp::Equality));
        }
        if self.admit_atomic(right, StructuralIdentityOp::Equality)? {
            return left.equals(right);
        }
        if Struct::from_value(right).is_some()
            || ListRef::from_value(right).is_some()
            || DictRef::from_value(right).is_some()
            || TupleRef::from_value(right).is_some()
        {
            return Ok(false);
        }
        if let Some(equal) = native.equals(self, right, left)? {
            return Ok(equal);
        }
        Err(self.unsupported(right, StructuralIdentityOp::Equality))
    }

    fn equal_sequence(&mut self, left: &[Value<'v>], right: &[Value<'v>]) -> crate::Result<bool> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for (left, right) in left.iter().zip(right) {
            if !self.equals(*left, *right)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn unhashable(value: Value<'_>) -> crate::Error {
    crate::Error::new_other(ControlError::NotHashableValue(value.get_type().to_owned()))
}

#[cfg(test)]
mod tests {
    use starlark_map::Hashed;
    use starlark_map::StarlarkHashValue;
    use starlark_map::small_map::SmallMap;

    use super::*;
    use crate::environment::Globals;
    use crate::environment::LibraryExtension;
    use crate::environment::Module;
    use crate::eval::Evaluator;
    use crate::syntax::AstModule;
    use crate::syntax::Dialect;
    use crate::values::Heap;
    use crate::values::dict::AllocHashableDict;
    use crate::values::dict::Dict;
    use crate::values::structs::starlark_structural_is_immutable;
    use crate::values::structs::write_starlark_structural_hash;

    struct NoNative;

    impl<'v> StructuralIdentityNative<'v> for NoNative {
        fn admit_atomic(
            &self,
            _: Value<'v>,
            _: StructuralIdentityOp,
            _: &ExecutionBudget,
        ) -> Result<bool, ExecutionLimit> {
            Ok(false)
        }

        fn is_immutable(
            &self,
            _: &mut StructuralIdentity<'_, 'v, Self>,
            _: Value<'v>,
        ) -> crate::Result<Option<bool>> {
            Ok(None)
        }

        fn write_hash(
            &self,
            _: &mut StructuralIdentity<'_, 'v, Self>,
            _: Value<'v>,
            _: &mut StarlarkHasher,
            _: bool,
        ) -> crate::Result<bool> {
            Ok(false)
        }

        fn equals(
            &self,
            _: &mut StructuralIdentity<'_, 'v, Self>,
            _: Value<'v>,
            _: Value<'v>,
        ) -> crate::Result<Option<bool>> {
            Ok(None)
        }
    }

    fn eval<'v>(module: &'v Module, source: &str) -> Value<'v> {
        let ast = AstModule::parse("identity.star", source.to_owned(), &Dialect::Standard).unwrap();
        let globals = Globals::extended_by(&[LibraryExtension::StructType]);
        Evaluator::new(module).eval_module(ast, &globals).unwrap()
    }

    #[test]
    fn hash_modes_match_ordinary_framing_and_unordered_members() {
        let module = Module::new();
        eval(
            &module,
            r#"
record_a = struct(z = {"k": ["same", (2, "two")], "b": [3]}, a = (True, 2.5))
record_b = struct(a = (True, 2.5), z = {"b": [3], "k": ["same", (2, "two")]})
scalar = "string"
sequence = (True, "two", 2, 2.0, None)
structural_dict = {"3": [4], "1": [2], "2": [3], "0": [1], "4": [5]}
"#,
        );
        let frozen = module.freeze().unwrap();
        let module = Module::new();
        let heap = module.heap();
        let budget = ExecutionBudget::new(16_384, 16_384, 16);
        let mut ctx = StructuralIdentity::new(&budget, &NoNative);
        let get = |name| frozen.get(name).unwrap().owned_value(module.frozen_heap());
        let record_a = get("record_a");
        let record_b = get("record_b");
        assert!(ctx.equals(record_a, record_b).unwrap());
        assert_eq!(
            ctx.leaf_hash(record_a).unwrap(),
            ctx.leaf_hash(record_b).unwrap()
        );
        let dict = heap.alloc(AllocHashableDict([("z", 2), ("a", 1)]));
        for value in [record_a, record_b, get("scalar"), get("sequence"), dict] {
            assert_eq!(
                ctx.leaf_hash(value).unwrap(),
                u64::from(value.get_hashed().unwrap().hash().get()),
            );
        }
        for value in [
            record_a,
            record_b,
            get("sequence"),
            dict,
            get("structural_dict"),
        ] {
            let mut expected = StarlarkHasher::new();
            let mut observed = StarlarkHasher::new();
            write_starlark_structural_hash(value, &mut expected).unwrap();
            ctx.write_hash(value, &mut observed, true).unwrap();
            assert_eq!(expected.finish(), observed.finish());
            assert_eq!(
                ctx.is_immutable(value).unwrap(),
                starlark_structural_is_immutable(value)
            );
        }
    }

    fn collision_dict<'v>(heap: Heap<'v>, entries: &[(Value<'v>, Value<'v>)]) -> Value<'v> {
        let mut map = SmallMap::new();
        for &(key, value) in entries {
            map.insert_hashed(
                Hashed::new_unchecked(StarlarkHashValue::new_unchecked(7), key),
                value,
            );
        }
        heap.alloc(Dict::new(map))
    }

    #[test]
    fn dictionary_key_collisions_keep_exact_equality_and_can_refuse_work() {
        let module = Module::new();
        let heap = module.heap();
        let key_a = heap.alloc("first");
        let key_b = heap.alloc("second");
        let one = heap.alloc(1);
        let two = heap.alloc(2);
        let left = collision_dict(heap, &[(key_a, one), (key_b, two)]);
        let equal = collision_dict(heap, &[(key_b, two), (key_a, one)]);
        let wrong = collision_dict(heap, &[(key_b, one), (key_a, two)]);
        let budget = ExecutionBudget::new(128, 0, 8);
        let mut ctx = StructuralIdentity::new(&budget, &NoNative);
        assert!(ctx.equals(left, equal).unwrap());
        assert!(!ctx.equals(left, wrong).unwrap());

        let budget = ExecutionBudget::new(2, 0, 8);
        let mut ctx = StructuralIdentity::new(&budget, &NoNative);
        assert!(ctx.equals(left, equal).is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::Work));
    }

    #[test]
    fn depth_cycles_and_unadmitted_hash_do_not_fall_back() {
        let module = Module::new();
        eval(
            &module,
            r#"
left = []
left.append(left)
right = []
right.append(right)
sequence = (((1,),),)
unadmitted = range(1)
"#,
        );
        let budget = ExecutionBudget::new(256, 256, 8);
        let left = module.get("left").unwrap();
        let right = module.get("right").unwrap();
        let mut ctx = StructuralIdentity::new(&budget, &NoNative);
        assert!(ctx.equals(left, left).unwrap());
        assert!(ctx.equals(left, right).is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::StructuralCycle));

        let budget = ExecutionBudget::new(256, 256, 2);
        let mut ctx = StructuralIdentity::new(&budget, &NoNative);
        assert!(ctx.leaf_hash(module.get("sequence").unwrap()).is_err());
        assert_eq!(budget.check(), Err(ExecutionLimit::StructuralDepth));

        let budget = ExecutionBudget::new(256, 256, 8);
        let mut ctx = StructuralIdentity::new(&budget, &NoNative);
        assert!(ctx.leaf_hash(module.get("unadmitted").unwrap()).is_err());
        assert_eq!(
            budget.check(),
            Err(ExecutionLimit::UnsupportedOperation {
                operation: "structural hashing",
                value_type: "range",
            })
        );
    }
}
