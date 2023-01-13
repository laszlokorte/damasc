use std::{collections::{HashMap, BTreeSet}, borrow::Cow};

use crate::{identifier::Identifier, typed_bag::{TypedBag, TypedTransfer}, value::Value, env::{Environment, EvalError}, query::{UpdateQuery, DeletionQuery, Predicate, ProjectionQuery, TransferQuery}, bag::Completion};

#[derive(Clone)]
pub struct BagBundle<'b, 'i, 's, 'v> {
    pub bags: HashMap<Identifier<'s>, Cow<'b, TypedBag<'i, 's, 'v>>>,
}

impl<'b, 'i, 's, 'v> BagBundle<'b, 'i, 's, 'v> {
    pub(crate) fn new() -> Self {
        Self {
            bags: HashMap::new()
        }
    }

    pub(crate) fn bag_names(&self) -> BTreeSet<Identifier<'v>> {
        self.bags.keys().cloned().collect()
    }

    fn insert(&mut self, 
        bag: &Identifier<'s>, 
        values: impl Iterator<Item=Value<'s, 'v>>) -> Result<(Completion, usize), BagBundleError> {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        let mut counter = 0;

        for v in values {
            if bag.to_mut().insert(&v) {
                counter+=1;
            } else {
                return Ok((Completion::Partial, counter))
            }
        }

        Ok((Completion::Complete, counter))
    }

    fn update<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>) -> Result<(Completion, usize), BagBundleError>  {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.to_mut().update(env, update))
    }

    fn delete<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>) -> Result<(Completion, usize), BagBundleError> {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.to_mut().delete(env, deletion))
    }

    fn create_bag(&mut self, bag_name: Identifier<'s>, predicate: Predicate<'s>) -> Result<(), BagBundleError> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.bags.entry(bag_name) {
            e.insert(Cow::Owned(TypedBag::new(predicate)));
            Ok(())
        } else {
            Err(BagBundleError::BagAlreadyExists)
        }
    }

    fn get_bag_info(&self, bag: &Identifier<'s>) -> Result<(usize, &Predicate), BagBundleError> {
        let Some(bag) = self.bags.get(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok((bag.len(), &bag.guard))
    }

    fn read<'x>(&'x self, bag: &'x Identifier) -> Result<impl Iterator<Item = &Cow<'v, Value<'s, 'v>>>, BagBundleError> {
        let Some(bag) = self.bags.get(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.iter())
    }

    fn query<'e, 'x: 'e>(
        &'x self,
        bag: &'x Identifier,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e ProjectionQuery<'s>,
    ) -> Result<impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e, BagBundleError> {
        let Some(bag) = self.bags.get(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.query(env, query))
    }

    fn transfer<'e>(&mut self, source: &Identifier<'s>, sink: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        query: TransferQuery<'s>) -> Result<(Completion, usize), BagBundleError> {
        let Some([bag_from, bag_to]) = self.bags.get_many_mut([source, sink]) else {
            return Err(BagBundleError::BagDoesNotExist);
        };

        let mut transfer = TypedTransfer::new(bag_from.to_mut(), bag_to.to_mut());
        
        Ok(transfer.transfer(env, &query))
    }

    fn pop<'x>(&mut self, bag: &Identifier<'s>, value: &'x Value<'s,'v>) -> Result<bool, BagBundleError> {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.to_mut().pop(value))
    }
}

pub(crate) enum BagBundleError{
    BagAlreadyExists,
    BagDoesNotExist,
    OperationError,
}
pub(crate) enum Transaction<'b, 'i, 's, 'v> {
    Clean {
        working_copy: Cow<'b, BagBundle<'b, 'i, 's, 'v>>
    },
    Dirty {
        working_copy: Cow<'b, BagBundle<'b, 'i, 's, 'v>>
    },
    Failed
}

pub(crate) enum TransactionError<E> {
    Aborted,
    Failed(E),
}

impl<'b, 'i, 's, 'v> Transaction<'b, 'i, 's, 'v> {
    fn get_working_copy<E:Sized>(&self) -> Result<&Cow<'b, BagBundle<'b, 'i, 's, 'v>>, TransactionError<E>> {
        match self {
            Transaction::Clean { working_copy } => Ok(working_copy),
            Transaction::Dirty { working_copy } => Ok(working_copy),
            Transaction::Failed => Err(TransactionError::Aborted),
        }
    }

    fn get_working_copy_mut<E:Sized>(&mut self) -> Result<&mut Cow<'b, BagBundle<'b, 'i, 's, 'v>>, TransactionError<E>> {
        match self {
            Transaction::Clean { working_copy } => Ok(working_copy),
            Transaction::Dirty { working_copy } => Ok(working_copy),
            Transaction::Failed => Err(TransactionError::Aborted),
        }
    }

    pub fn new(snapshot: &BagBundle<'b, 'i, 's, 'v>) -> Self {
        Self::Clean {
            working_copy: Cow::Owned(snapshot.clone()),
        }
    }

    pub(crate) fn bag_names<T:Sized>(&self) -> Result<BTreeSet<Identifier<'v>>, TransactionError<T>> {
        let working_copy = self.get_working_copy()?;

        Ok(working_copy.bag_names())
    }
    
    fn fail_or_dirty<A,B>(&mut self, result: Result<A,B>) -> Result<A,B> {
        if result.is_err() {
            *self = Self::Failed;
        } else if let Self::Clean {working_copy: wc} = self {
            *self = Self::Dirty { working_copy: wc.clone() }
        }

        result
    } 

    pub(crate) fn insert(&mut self, 
        bag: &Identifier<'s>, 
        values: impl Iterator<Item=Value<'s, 'v>>) -> Result<usize, TransactionError<BagBundleError>> {
        let working_copy = self.get_working_copy_mut()?;

        let result = working_copy.to_mut().insert(bag, values).and_then(|(completion, size)| {
            match completion {
                Completion::Complete => Ok(size),
                Completion::Partial => Err(BagBundleError::OperationError)
            }
        }).map_err(TransactionError::Failed);

        self.fail_or_dirty(result)
    }

    pub(crate) fn update<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>) -> Result<usize, TransactionError<BagBundleError>>  {
        let working_copy = self.get_working_copy_mut()?;

        let result = working_copy.to_mut().update(bag, env, update).and_then(|(completion, size)| {
            match completion {
                Completion::Complete => Ok(size),
                Completion::Partial => Err(BagBundleError::OperationError)
            }
        }).map_err(TransactionError::Failed);
        
        self.fail_or_dirty(result)
    }

    pub(crate) fn delete<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>) -> Result<usize, TransactionError<BagBundleError>> {
        
        let working_copy = self.get_working_copy_mut()?;

        let result = working_copy.to_mut().delete(bag, env, deletion).and_then(|(completion, size)| {
            match completion {
                Completion::Complete => Ok(size),
                Completion::Partial => Err(BagBundleError::OperationError)
            }
        }).map_err(TransactionError::Failed);

        self.fail_or_dirty(result)
    }

    pub(crate) fn create_bag(&mut self, bag_name: Identifier<'s>, predicate: Predicate<'s>) -> Result<(), TransactionError<BagBundleError>> {
        let working_copy = self.get_working_copy_mut()?;

        let result = working_copy.to_mut().create_bag(bag_name, predicate).map_err(TransactionError::Failed);

        self.fail_or_dirty(result)
    }

    pub(crate) fn get_bag_info(&mut self, bag: &Identifier<'s>) -> Result<(usize, &Predicate), TransactionError<BagBundleError>> {
        let working_copy = self.get_working_copy()?;

        working_copy.get_bag_info(bag).map_err(TransactionError::Failed)
    }

    pub(crate) fn read<'x>(&'x self, bag: &'x Identifier) -> Result<impl Iterator<Item = &Cow<'v, Value<'s, 'v>>>, TransactionError<BagBundleError>> {
        let working_copy = self.get_working_copy()?;

        working_copy.read(bag).map_err(TransactionError::Failed)
    }

    pub(crate) fn query<'e, 'x: 'e>(
        &'x self,
        bag: &'x Identifier,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e ProjectionQuery<'s>,
    ) -> Result<impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e, TransactionError<BagBundleError>> {
        let working_copy = self.get_working_copy()?;

        working_copy.query(bag, env, query).map_err(TransactionError::Failed)
    }

    pub(crate) fn transfer<'e>(&mut self, source: &Identifier<'s>, sink: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        query: TransferQuery<'s>) -> Result<usize, TransactionError<BagBundleError>> {
        
        let working_copy = self.get_working_copy_mut()?;

        let result = working_copy.to_mut().transfer(source, sink, env, query).and_then(|(completion, size)| {
            match completion {
                Completion::Complete => Ok(size),
                Completion::Partial => Err(BagBundleError::OperationError)
            }
        }).map_err(TransactionError::Failed);

        self.fail_or_dirty(result)
    }

    pub(crate) fn pop<'x>(&mut self, bag: &Identifier<'s>, value: &'x Value<'s,'v>) -> Result<bool, TransactionError<BagBundleError>> {
        let working_copy = self.get_working_copy_mut()?;

        let result = working_copy.to_mut().pop(bag, value).map_err(TransactionError::Failed);

        self.fail_or_dirty(result)
    }

    pub(crate) fn commit(self) -> Result<BagBundle<'b, 'i, 's, 'v>, TransactionError2> {
        match self {
            Transaction::Clean { working_copy } => Ok(working_copy.as_ref().to_owned()),
            Transaction::Dirty { working_copy } => Ok(working_copy.as_ref().to_owned()),
            Transaction::Failed => Err(TransactionError2::Aborted),
        }
    }
}


pub(crate) enum TransactionError2 {
    Aborted,
}