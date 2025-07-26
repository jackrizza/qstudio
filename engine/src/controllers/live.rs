use crate::parser::Query; // Update the path to where Query is defined

pub struct LiveController<'a> {
    query: &'a Query,
}
