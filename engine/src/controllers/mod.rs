pub mod fundamentals;
pub mod historical;
pub mod live;

use crate::parser::Graph;

pub enum Output {
    Graph(Graph),
    DataFrame(polars::frame::DataFrame),
}
