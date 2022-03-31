use crossterm::event::Event;

pub trait Screenlet {
    type Data;

    fn read(&self) -> Self::Data;
    fn event(&mut self, e: Event);
    fn update(&mut self);
}
