pub enum State {
  Ready,
  Running,
  Exiting
}

pub struct Process {
  pub id: u32,
  pub state: State,
  pub stack_start: usize,
  pub stack_end: usize,
  pub stack_ptr: usize
}