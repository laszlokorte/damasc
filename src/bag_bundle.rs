use std::{collections::{HashMap, BTreeSet}, borrow::Cow};

use crate::{identifier::Identifier, typed_bag::{TypedBag, TypedTransfer}, value::Value, env::{Environment, EvalError}, query::{UpdateQuery, DeletionQuery, Predicate, ProjectionQuery, TransferQuery}};

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
        values: impl Iterator<Item=Value<'s, 'v>>) -> Result<usize, BagBundleError> {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        let mut counter = 0;

        for v in values {
            if bag.to_mut().insert(&v) {
                counter+=1;
            }
        }

        Ok(counter)
    }

    fn update<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>) -> Result<usize, BagBundleError>  {
        let Some(bag) = self.bags.get_mut(bag) else {
            return Err(BagBundleError::BagDoesNotExist)
        };

        Ok(bag.to_mut().update(env, update))
    }

    fn delete<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>) -> Result<usize, BagBundleError> {
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
        query: TransferQuery<'s>) -> Result<usize, BagBundleError> {
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
}
pub(crate) struct Transaction<'b, 'i, 's, 'v> {
    working_copy: Cow<'b, BagBundle<'b, 'i, 's, 'v>>,
}

impl<'b, 'i, 's, 'v> Transaction<'b, 'i, 's, 'v> {
    fn new(snapshot: &BagBundle<'b, 'i, 's, 'v>) -> Self {
        Self {
            working_copy: Cow::Owned(snapshot.clone()),
        }
    }

    pub(crate) fn run<F,T,E>(snapshot: &mut BagBundle<'b, 'i, 's, 'v>, f: F) -> Result<T,E> where F: FnOnce(&mut Self)->Result<T,E> {
        let mut trans = Transaction::new(snapshot);
        let r = f(&mut trans)?;
        *snapshot = trans.commit();
        Ok(r)
    }

    pub(crate) fn bag_names(&self) -> BTreeSet<Identifier<'v>> {
        self.working_copy.bag_names()
    }

    pub(crate) fn insert(&mut self, 
        bag: &Identifier<'s>, 
        values: impl Iterator<Item=Value<'s, 'v>>) -> Result<usize, BagBundleError> {
        
        self.working_copy.to_mut().insert(bag, values)
    }

    pub(crate) fn update<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        update: &'e UpdateQuery<'s>) -> Result<usize, BagBundleError>  {
        
        self.working_copy.to_mut().update(bag, env, update)
    }

    pub(crate) fn delete<'e>(&mut self, 
        bag: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        deletion: &'e DeletionQuery<'s>) -> Result<usize, BagBundleError> {
        

        self.working_copy.to_mut().delete(bag, env, deletion)
    }

    pub(crate) fn create_bag(&mut self, bag_name: Identifier<'s>, predicate: Predicate<'s>) -> Result<(), BagBundleError> {
        self.working_copy.to_mut().create_bag(bag_name, predicate)
    }

    pub(crate) fn get_bag_info(&self, bag: &Identifier<'s>) -> Result<(usize, &Predicate), BagBundleError> {
        self.working_copy.get_bag_info(bag)
    }

    pub(crate) fn read<'x>(&'x self, bag: &'x Identifier) -> Result<impl Iterator<Item = &Cow<'v, Value<'s, 'v>>>, BagBundleError> {
        self.working_copy.read(bag)
    }

    pub(crate) fn query<'e, 'x: 'e>(
        &'x self,
        bag: &'x Identifier,
        env: &'e Environment<'i, 's, 'v>,
        query: &'e ProjectionQuery<'s>,
    ) -> Result<impl Iterator<Item = Result<Value<'s, 'v>, EvalError>> + 'e, BagBundleError> {
        self.working_copy.query(bag, env, query)
    }

    pub(crate) fn transfer<'e>(&mut self, source: &Identifier<'s>, sink: &Identifier<'s>, 
        env: &'e Environment<'i, 's, 'v>,
        query: TransferQuery<'s>) -> Result<usize, BagBundleError> {
        
        self.working_copy.to_mut().transfer(source, sink, env, query)
    }

    pub(crate) fn pop<'x>(&mut self, bag: &Identifier<'s>, value: &'x Value<'s,'v>) -> Result<bool, BagBundleError> {
        self.working_copy.to_mut().pop(bag, value)
    }

    pub(crate) fn commit(self) -> BagBundle<'b, 'i, 's, 'v> {
        self.working_copy.as_ref().to_owned()
    }
}