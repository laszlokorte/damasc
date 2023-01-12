use std::{collections::{HashMap, BTreeSet}, borrow::Cow};

use crate::{identifier::Identifier, typed_bag::{TypedBag, TypedTransfer}, value::Value, env::{Environment, EvalError}, query::{UpdateQuery, DeletionQuery, Predicate, ProjectionQuery, TransfereQuery}};

pub struct BagBundle<'i, 's, 'v> {
    pub bags: HashMap<Identifier<'s>, TypedBag<'i, 's, 'v>>,
}

impl<'i, 's, 'v> BagBundle<'i, 's, 'v> {
    pub(crate) fn new() -> Self {
        Self {
            bags: HashMap::new()
        }
    }

    pub fn bag_names(&self) -> BTreeSet<Identifier<'v>> {
        self.bags.keys().cloned().collect()
    }

    pub(crate) fn insert(&mut self, 
        bag: &Identifier<'s>, 
        values: impl Iterator<Item=Value<'s, 'v>>) -> Result<usize, BagBundleError> {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        let mut counter = 0;

        for v in values {
            if bag.insert(&v) {
                counter+=1;
            }
        }

        Ok(counter)
    }

    pub(crate) fn update<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>) -> Result<usize, BagBundleError>  {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.update(env, update))
    }

    pub(crate) fn delete<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>) -> Result<usize, BagBundleError> {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.delete(env, deletion))
    }

    pub(crate) fn create_bag(&mut self, bag_name: Identifier<'s>, predicate: Predicate<'s>) -> Result<(), BagBundleError> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.bags.entry(bag_name) {
            e.insert(TypedBag::new(predicate));
            Ok(())
        } else {
            Err(BagBundleError::BagAlreadyExists)
        }
    }

    pub(crate) fn get_bag_info(&self, bag: &Identifier<'s>) -> Result<(usize, &Predicate), BagBundleError> {
        let Some(bag) = self.bags.get(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok((bag.len(), &bag.guard))
    }

    pub(crate) fn read<'x>(&'x self, bag: &'x Identifier) -> Result<impl Iterator<Item = &Cow<'v, Value<'s, 'v>>>, BagBundleError> {
        let Some(bag) = self.bags.get(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        

        Ok(bag.iter())
    }

    pub(crate) fn query<'e, 'x: 'e>(
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

    pub(crate) fn transfere<'e>(&mut self, source: &Identifier<'s>, sink: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        query: TransfereQuery<'s>) -> Result<usize, BagBundleError> {
        let Some([bag_from, bag_to]) = self.bags.get_many_mut([source, sink]) else {
            return Err(BagBundleError::BagDoesNotExist);
        };

        let mut transfer = TypedTransfer::new(bag_from, bag_to);
        
        Ok(transfer.transfer(env, &query))

    }

    pub(crate) fn pop<'x>(&mut self, bag: &Identifier<'s>, value: &'x Value<'s,'v>) -> Result<bool, BagBundleError> {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.pop(value))
    }
}

pub(crate) enum BagBundleError{
    BagAlreadyExists,
    BagDoesNotExist,
}
