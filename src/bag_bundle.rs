use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
};

use crate::bag::DeletionResult;
use crate::bag::TransferResult;
use crate::{
    bag::{InsertionResult, UpdateResult},
    bag::{ValueBag, ValueBagTransfer},
    env::{Environment, EvalError},
    expression::Expression,
    identifier::Identifier,
    query::{DeletionQuery, Insertion, Predicate, ProjectionQuery, TransferQuery, UpdateQuery},
    value::Value,
};

#[derive(Clone)]
pub struct BagBundle<'b, 'i, 's, 'v> {
    pub bags: HashMap<Identifier<'s>, Cow<'b, ValueBag<'i, 's, 'v>>>,
}

impl<'b, 'i, 's, 'v> BagBundle<'b, 'i, 's, 'v> {
    pub(crate) fn new() -> Self {
        Self {
            bags: HashMap::new(),
        }
    }

    pub(crate) fn bag_names(&self) -> BTreeSet<Identifier<'v>> {
        self.bags.keys().cloned().collect()
    }
}

pub(crate) enum Transaction<'b, 'i, 's, 'v> {
    Clean {
        working_copy: Cow<'b, BagBundle<'b, 'i, 's, 'v>>,
    },
    Failed,
}

#[derive(Debug)]
pub(crate) enum TransactionError {
    BagDoesNotExist,
    Aborted,
}

impl<'b, 'i, 's, 'v> Transaction<'b, 'i, 's, 'v> {
    fn get_working_copy(&self) -> Result<&Cow<'b, BagBundle<'b, 'i, 's, 'v>>, TransactionError> {
        match self {
            Transaction::Clean { working_copy } => Ok(working_copy),
            Transaction::Failed => Err(TransactionError::Aborted),
        }
    }

    fn get_working_copy_mut(
        &mut self,
    ) -> Result<&mut Cow<'b, BagBundle<'b, 'i, 's, 'v>>, TransactionError> {
        match self {
            Transaction::Clean { working_copy } => Ok(working_copy),
            Transaction::Failed => Err(TransactionError::Aborted),
        }
    }

    pub fn new(snapshot: &BagBundle<'b, 'i, 's, 'v>) -> Self {
        Self::Clean {
            working_copy: Cow::Owned(snapshot.clone()),
        }
    }

    pub(crate) fn bag_names(&self) -> Result<BTreeSet<Identifier<'v>>, TransactionError> {
        let working_copy = self.get_working_copy()?;

        Ok(working_copy.bag_names())
    }

    pub(crate) fn insert<'e>(
        &mut self,
        bag_name: &Identifier<'s>,
        env: &'e Environment<'i, 's, 'v>,
        insertion: &Insertion<'s>,
    ) -> Result<InsertionResult, TransactionError> {
        let working_copy = self.get_working_copy_mut()?;
        let Some(bag) = working_copy.to_mut().bags.get_mut(bag_name) else {
            *self = Self::Failed;
            return Err(TransactionError::BagDoesNotExist)
        };

        Ok(bag.to_mut().insert(env, insertion))
    }

    pub(crate) fn update<'e>(
        &mut self,
        bag: &Identifier<'s>,
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>,
    ) -> Result<UpdateResult, TransactionError> {
        let working_copy = self.get_working_copy_mut()?;
        let Some(bag) = working_copy.to_mut().bags.get_mut(bag) else {
            *self = Self::Failed;
            return Err(TransactionError::BagDoesNotExist)
        };

        Ok(bag.to_mut().update(env, update))
    }

    pub(crate) fn delete<'e>(
        &mut self,
        bag: &Identifier<'s>,
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>,
    ) -> Result<DeletionResult, TransactionError> {
        let working_copy = self.get_working_copy_mut()?;
        let Some(bag) = working_copy.to_mut().bags.get_mut(bag) else {
            *self = Self::Failed;
            return Err(TransactionError::BagDoesNotExist)
        };

        Ok(bag.to_mut().delete(env, deletion))
    }

    pub(crate) fn create_bag(
        &mut self,
        bag_name: Identifier<'s>,
        predicate: Predicate<'s>,
    ) -> Result<bool, TransactionError> {
        let working_copy = self.get_working_copy_mut()?;
        
        if let std::collections::hash_map::Entry::Vacant(e) =
            working_copy.to_mut().bags.entry(bag_name)
        {
            e.insert(Cow::Owned(ValueBag::new(predicate)));

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) fn get_bag_info(
        &mut self,
        bag_name: &Identifier<'s>,
    ) -> Result<(usize, &Predicate), TransactionError> {
        let working_copy = self.get_working_copy()?;

        let Some(b) = working_copy.bags.get(bag_name) else {
            return Err(TransactionError::BagDoesNotExist)
        };

        Ok((b.len(), &b.guard))
    }

    pub(crate) fn read<'x>(
        &'x self,
        bag_name: &'x Identifier,
    ) -> Result<impl Iterator<Item = &Cow<'v, Value<'s, 'v>>>, TransactionError> {
        let working_copy = self.get_working_copy()?;

        let Some(b) = working_copy.bags.get(bag_name) else {
            return Err(TransactionError::BagDoesNotExist);
        };

        Ok(b.iter())
    }

    pub(crate) fn query<'e, 'x: 'e>(
        &'x self,
        bag_name: &'x Identifier,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e ProjectionQuery<'s>,
    ) -> Result<impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e, TransactionError> {
        let working_copy = self.get_working_copy()?;

        let Some(b) = working_copy.bags.get(bag_name) else {
            return Err(TransactionError::BagDoesNotExist);
        };

        Ok(b.query(env, query))
    }

    pub(crate) fn transfer<'e>(
        &mut self,
        source: &Identifier<'s>,
        sink: &Identifier<'s>,
        env: &'e Environment<'i, 's, 'v>,
        query: TransferQuery<'s>,
    ) -> Result<TransferResult, TransactionError> {
        let working_copy = self.get_working_copy_mut()?;
        let Some([a,b]) = working_copy.to_mut().bags.get_many_mut([source, sink]) else {
            return Err(TransactionError::BagDoesNotExist);
        };

        let a = a.to_mut();
        let b = b.to_mut();

        let mut trans = ValueBagTransfer::new(a, b);

        Ok(trans.transfer(env, &query))
    }

    pub(crate) fn pop<'x>(
        &mut self,
        bag_name: &Identifier<'s>,
        value: &'x Value<'s, 'v>,
    ) -> Result<bool, TransactionError> {
        let working_copy = self.get_working_copy_mut()?;

        let Some(b) = working_copy.to_mut().bags.get_mut(bag_name) else {
            return Err(TransactionError::BagDoesNotExist);
        };

        Ok(b.to_mut().pop(value))
    }

    pub(crate) fn commit(self) -> Result<BagBundle<'b, 'i, 's, 'v>, TransactionError> {
        match self {
            Transaction::Clean { working_copy } => Ok(working_copy.as_ref().to_owned()),
            Transaction::Failed => Err(TransactionError::Aborted),
        }
    }

    pub(crate) fn insert_one<'e, 'x: 'e>(
        &mut self,
        bag_name: &Identifier<'s>,
        env: &'e Environment<'i, 's, 'v>,
        expr: &'x Expression<'s>,
    ) -> Result<InsertionResult, TransactionError> {
        let working_copy = self.get_working_copy_mut()?;

        let Some(b) = working_copy.to_mut().bags.get_mut(bag_name) else {
            return Err(TransactionError::BagDoesNotExist);
        };

        Ok(b.to_mut().insert_one(env, expr))
    }
}
