use lwk_simplicity_options::simplicityhl;

/// Log level for Simplicity program execution tracing.
#[derive(uniffi::Enum, Clone, Copy, Debug, Default)]
pub enum SimplicityLogLevel {
    /// No output during execution.
    #[default]
    None,
    /// Print debug information.
    Debug,
    /// Print debug and warning information.
    Warning,
    /// Print debug, warning, and jet execution trace.
    Trace,
}

impl From<SimplicityLogLevel> for simplicityhl::tracker::TrackerLogLevel {
    fn from(level: SimplicityLogLevel) -> Self {
        match level {
            SimplicityLogLevel::None => simplicityhl::tracker::TrackerLogLevel::None,
            SimplicityLogLevel::Debug => simplicityhl::tracker::TrackerLogLevel::Debug,
            SimplicityLogLevel::Warning => simplicityhl::tracker::TrackerLogLevel::Warning,
            SimplicityLogLevel::Trace => simplicityhl::tracker::TrackerLogLevel::Trace,
        }
    }
}
