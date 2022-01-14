use std::borrow::{Borrow, BorrowMut};
use std::cell::RefMut;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use crate::{impl_helper::*, prelude::*};

use std::time::{Duration};
use wasm_bindgen::prelude::*;
use wasm_timer::Instant;

#[wasm_bindgen]
extern "C" {
  fn setInterval(closure: &Closure<dyn FnMut()>, millis: u32) -> JsValue;
  fn clearInterval(token: &JsValue);

  #[wasm_bindgen(js_namespace = console)]
  fn log(s: &str);
}

/// Creates an observable which will fire at `dur` time into the future,
/// and will repeat every `dur` interval after.
pub fn js_interval(dur: Duration) -> JsIntervalObservable {
  JsIntervalObservable {
    dur,
    at: None,
  }
}

/// Creates an observable which will fire at the time specified by `at`,
/// and then will repeat every `dur` interval after
pub fn js_interval_at(
  at: Instant,
  dur: Duration,
) -> JsIntervalObservable {
  JsIntervalObservable {
    dur,
    at: Some(at),
  }
}

#[derive(Clone)]
pub struct JsIntervalObservable {
  dur: Duration,
  at: Option<Instant>,
}

impl Observable for JsIntervalObservable {
  type Item = usize;
  type Err = ();
}

pub struct IntervalId(f64);

pub struct IntervalHandle<F: ?Sized> {
  pub token: JsValue,
  _closure: Closure<F>,
  is_closed: Arc<RwLock<bool>>,
}

impl<F: ?Sized> IntervalHandle<F> {
  #[inline]
  pub fn new(token: JsValue, _closure: Closure<F>) -> Self {
    Self {
      token,
      _closure,
      is_closed: Arc::new(RwLock::new(false)),
    }
  }
}

impl<F: ?Sized> SubscriptionLike for IntervalHandle<F> {
  fn unsubscribe(&mut self) {
    let was_closed = *self.is_closed.read().unwrap();
    if !was_closed {
      *self.is_closed.write().unwrap() = true;
      log(format!("closing interval : {}", self.token.as_f64().unwrap()).as_str());
      clearInterval(&self.token)
    }
  }

  #[inline]
  fn is_closed(&self) -> bool { *self.is_closed.read().unwrap() }
}

impl LocalObservable<'static> for JsIntervalObservable
{
  type Unsub = IntervalHandle<dyn FnMut()>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub where O: Observer<Item=Self::Item, Err=Self::Err> + 'static {
    let counter: MutRc<usize> = MutRc::own(0);
    let mut observer = observer;
    let closure = Closure::new(move || {
      let mut num = counter.rc_deref_mut();
      observer.next(*num);
      *num = *num + 1;
    });
    let token = setInterval(&closure, self.dur.as_millis() as u32);
    log(format!("token: {}", token.as_f64().unwrap()).as_str());
    IntervalHandle::new(token, closure)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_scheduler::ManualScheduler;
  use futures::executor::{LocalPool, ThreadPool};
  use std::sync::{Arc, Mutex};

  #[test]
  fn local() {
    let mut local = LocalPool::new();
    let stamp = Instant::now();
    let ticks = Arc::new(Mutex::new(0));
    let ticks_c = Arc::clone(&ticks);
    interval(Duration::from_millis(1), local.spawner())
      .take(5)
      .subscribe(move |_| (*ticks_c.lock().unwrap()) += 1);
    local.run();
    assert_eq!(*ticks.lock().unwrap(), 5);
    assert!(stamp.elapsed() > Duration::from_millis(5));
  }

  #[test]
  fn local_manual() {
    let scheduler = ManualScheduler::now();
    let ticks = Arc::new(Mutex::new(0));
    let ticks_c = Arc::clone(&ticks);
    let delay = Duration::from_millis(1);
    interval(delay, scheduler.clone())
      .take(5)
      .subscribe(move |_| (*ticks_c.lock().unwrap()) += 1);
    assert_eq!(0, *ticks.lock().unwrap());
    scheduler.advance(delay * 2);
    scheduler.run_tasks();
    assert_eq!(2, *ticks.lock().unwrap());

    scheduler.advance(delay * 3);
    scheduler.run_tasks();
    assert_eq!(5, *ticks.lock().unwrap());
  }
}
