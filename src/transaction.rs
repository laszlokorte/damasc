use std::borrow::Cow;

use crate::bag_bundle::BagBundle;

struct Transaction<'b, 'i, 's, 'v> {
    snapshot: Cow<'b, BagBundle<'b, 'i, 's, 'v>>,
}

impl<'b, 'i, 's, 'v> Transaction<'b, 'i, 's, 'v> {
    fn new(bags: &'b BagBundle<'b, 'i, 's, 'v>) -> Self {
        Self {
            snapshot: Cow::Borrowed(bags),
        }
    }
}