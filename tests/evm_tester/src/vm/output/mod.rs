use event::Event;

pub mod event;

///
/// The compiler test outcome data.
///
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ExecutionOutput {
    /// The return data values.
    pub return_data: Vec<u8>,
    /// Whether an exception is thrown,
    pub exception: bool,
    /// The emitted events.
    pub events: Vec<Event>,
    pub system_error: Option<(usize, usize)>,
}

impl ExecutionOutput {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        return_data: Vec<u8>,
        exception: bool,
        events: Vec<Event>,
        system_error: Option<(usize, usize)>,
    ) -> Self {
        Self {
            return_data,
            exception,
            events,
            system_error,
        }
    }
}
