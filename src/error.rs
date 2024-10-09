#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Function {0} not found")]
    FunctionNotFound(String),
    #[error("Invalid arity for function {0}: got {1}, expected one of {2:?}")]
    InvalidArity(String, usize, Vec<usize>),
    #[error("{0}")]
    BuiltinFunctionError(String),
}
pub type Result<T> = std::result::Result<T, Error>;
