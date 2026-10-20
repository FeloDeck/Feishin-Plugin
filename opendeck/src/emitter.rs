// A small event emitter: listeners run right away, in the order they were added.
use std::cell::RefCell;
use std::rc::Rc;

type Listener<T> = Rc<dyn Fn(&T)>;

pub struct Emitter<T: ?Sized> {
	listeners: RefCell<Vec<Listener<T>>>,
}

impl<T: ?Sized> Default for Emitter<T> {
	fn default() -> Self {
		Emitter { listeners: RefCell::new(Vec::new()) }
	}
}

impl<T: ?Sized> Emitter<T> {
	pub fn on(&self, listener: impl Fn(&T) + 'static) {
		self.listeners.borrow_mut().push(Rc::new(listener));
	}

	/// The listeners at the time of the call all run, even if one adds another.
	pub fn emit(&self, value: &T) {
		let listeners: Vec<Listener<T>> = self.listeners.borrow().clone();
		for listener in listeners {
			listener(value);
		}
	}
}
