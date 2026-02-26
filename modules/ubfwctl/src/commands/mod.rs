//! Command implementations for ubfwctl

pub mod list;
pub mod mar_perf;

/// Trait for fwctl commands
///
/// This trait defines the interface for all fwctl-based commands.
/// Future commands can implement this trait for consistent execution.
pub trait FwctlCommand {
    /// Input type for the command
    type Input;
    /// Output type for the command
    type Output;
    /// Error type for the command
    type Error;

    /// Execute the command
    ///
    /// # Arguments
    /// * `input` - Input parameters for the command
    ///
    /// # Returns
    /// `Ok(Output)` on success, `Err(Error)` on failure
    ///
    /// # Errors
    /// Returns an error if command execution fails
    fn execute(
        &self,
        input: Self::Input,
    ) -> Result<Self::Output, Self::Error>;
}
