#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Function {0} not found")]
    FunctionNotFound(String),
    #[error("Invalid arity for function {0}: got {1}, expected one of {2:?}")]
    InvalidArity(String, usize, Vec<usize>),
    #[error("{0}")]
    BuiltinFunctionError(String),
    #[error("Ran >> on an empty sequence")]
    ShiftRightEmptySequence,
    #[error("Variable {0} not found")]
    VariableNotFound(String),
}
pub type Result<T> = std::result::Result<T, Error>;
