use crate::prelude::*;
use async_std::prelude::FutureExt as AsyncFutureExt;
use futures::future::{lazy, AbortHandle, FutureExt};
use std::future::Future;

use futures::StreamExt;
use std::sync::{Arc, RwLock};
use std::time::{Duration};
use wasm_timer::{Instant};

pub fn task_future<T>(
  task: impl FnOnce(T) + 'static,
  state: T,
  delay: Option<Duration>,
) -> (impl Future<Output = ()>, SpawnHandle) {
  let fut = lazy(|_| task(state)).delay(delay.unwrap_or_default());
  let (fut, handle) = futures::future::abortable(fut);
  (fut.map(|_| ()), SpawnHandle::new(handle))
}

/// A Scheduler is an object to order task and schedule their execution.
#[cfg(not(target_arch = "wasm32"))]
pub trait SharedScheduler {
  fn spawn<Fut>(&self, future: Fut)
  where
    Fut: Future<Output = ()> + Send + 'static;

  fn schedule<T: Send + 'static>(
    &self,
    task: impl FnOnce(T) + Send + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> SpawnHandle {
    let (f, handle) = task_future(task, state, delay);
    self.spawn(f);
    handle
  }

  fn schedule_repeating(
    &self,
    task: impl FnMut(usize) + Send + 'static,
    time_between: Duration,
    at: Option<Instant>,
  ) -> SpawnHandle {
    let (f, handle) = repeating_future(task, time_between, at);
    self.spawn(f.map(|_| ()));
    handle
  }
}

pub trait LocalScheduler {
  fn spawn<Fut>(&self, future: Fut)
  where
    Fut: Future<Output = ()> + 'static;

  fn schedule<T: 'static>(
    &self,
    task: impl FnOnce(T) + 'static,
    delay: Option<Duration>,
    state: T,
  ) -> SpawnHandle {
    let (f, handle) = task_future(task, state, delay);
    self.spawn(f);
    handle
  }

  fn schedule_repeating(
    &self,
    task: impl FnMut(usize) + 'static,
    time_between: Duration,
    at: Option<Instant>,
  ) -> SpawnHandle {
    let (f, handle) = repeating_future(task, time_between, at);
    self.spawn(f.map(|_| ()));
    handle
  }
}

#[derive(Clone)]
pub struct SpawnHandle {
  pub handle: AbortHandle,
  is_closed: Arc<RwLock<bool>>,
}

impl SpawnHandle {
  #[inline]
  pub fn new(handle: AbortHandle) -> Self {
    SpawnHandle {
      handle,
      is_closed: Arc::new(RwLock::new(false)),
    }
  }
}

impl SubscriptionLike for SpawnHandle {
  fn unsubscribe(&mut self) {
    let was_closed = *self.is_closed.read().unwrap();
    if !was_closed {
      *self.is_closed.write().unwrap() = true;
      self.handle.abort();
    }
  }

  #[inline]
  fn is_closed(&self) -> bool { *self.is_closed.read().unwrap() }
}

#[cfg(feature = "futures-scheduler")]
mod futures_scheduler {
  use crate::scheduler::LocalScheduler;
  #[cfg(not(target_arch = "wasm32"))]
  use crate::scheduler::SharedScheduler;
  use futures::{
    executor::LocalSpawner, task::LocalSpawnExt, Future, FutureExt,
  };
  #[cfg(not(target_arch = "wasm32"))]
  use futures::{executor::ThreadPool, task::SpawnExt};

  #[cfg(not(target_arch = "wasm32"))]
  impl SharedScheduler for ThreadPool {
    fn spawn<Fut>(&self, future: Fut)
    where
      Fut: Future<Output = ()> + Send + 'static,
    {
      SpawnExt::spawn(self, future).unwrap();
    }
  }

  impl LocalScheduler for LocalSpawner {
    fn spawn<Fut>(&self, future: Fut)
    where
      Fut: Future<Output = ()> + 'static,
    {
      self.spawn_local(future.map(|_| ())).unwrap();
    }
  }
}

#[cfg(target_arch = "wasm32")]
pub struct LocalSpawner;

#[cfg(all(target_arch = "wasm32", feature = "wasm-scheduler"))]
mod wasm_scheduler {
  use wasm_bindgen::prelude::*;
  use wasm_timer::{Instant};
  use crate::scheduler::{LocalScheduler, LocalSpawner, to_interval};
  use wasm_bindgen_futures::spawn_local;
  use crate::prelude::SpawnHandle;
  use futures::channel::oneshot;
  use std::future::Future;
  use std::time::Duration;
  use futures::future::{lazy, FutureExt};
  use async_std::prelude::FutureExt as AsyncFutureExt;

  #[wasm_bindgen]
  extern "C" {
    fn setInterval(closure: &Closure<dyn FnMut()>, millis: u32) -> f64;
    fn cancelInterval(token: f64);
  }


  impl LocalScheduler for LocalSpawner {
    fn spawn<Fut>(&self, future: Fut)
    where
      Fut: futures::Future<Output = ()> + 'static,
    {
      spawn_local(future.map(|_| {}));
    }

    fn schedule_repeating(&self, task: impl FnMut(usize) + 'static, time_between: Duration, at: Option<Instant>) -> SpawnHandle {
      let (f, handle) = repeating_future(task, time_between, at);
      self.spawn(f.map(|_| ()));
      handle
    }
  }
  //
  // fn fake_repeating_future(
  //   task: impl FnMut(usize) + 'static,
  //   time_between: Duration,
  //   _at: Option<Instant>,
  // ) -> (impl Future<Output=()>, SpawnHandle) {
  //   let mut task = task;
  //   let fut = lazy(move |_| task(42)).delay(time_between);
  //   let (fut, handle) = futures::future::abortable(fut);
  //   (fut.map(|_| ()), SpawnHandle::new(handle))
  // }

  fn repeating_future(
    task: impl FnMut(usize) + 'static,
    time_between: Duration,
    at: Option<Instant>,
  ) -> (impl Future<Output = ()>, SpawnHandle) {
    let now = Instant::now();
    let delay = at.map(|inst| {
      if inst > now {
        inst - now
      } else {
        Duration::from_micros(0)
      }
    });
    let future = to_interval(task, time_between, delay.unwrap_or(time_between));
    let (fut, handle) = futures::future::abortable(future);
    (fut.map(|_| ()), SpawnHandle::new(handle))
  }

  // fn to_interval(
  //   mut task: impl FnMut(usize) + 'static,
  //   interval_duration: Duration,
  //   _delay: Duration,
  // ) -> impl Future<Output = ()> {
  //   let mut number = 0;
  //
  //   futures::future::ready(())
  //       .then(move |_| {
  //         let (tx, rx) = oneshot::channel();
  //         task(number);
  //         let closure = Closure::new(move || {
  //           number += 1;
  //           task(number);
  //         });
  //         setInterval(&closure, interval_duration.as_millis() as u32);
  //         futures::future::ready(())
  //       })
  // }
}

fn repeating_future(
  task: impl FnMut(usize) + 'static,
  time_between: Duration,
  at: Option<Instant>,
) -> (impl Future<Output = ()>, SpawnHandle) {
  let now = Instant::now();
  let delay = at.map(|inst| {
    if inst > now {
      inst - now
    } else {
      Duration::from_micros(0)
    }
  });
  let future = to_interval(task, time_between, delay.unwrap_or(time_between));
  let (fut, handle) = futures::future::abortable(future);
  (fut.map(|_| ()), SpawnHandle::new(handle))
}

fn to_interval(
  mut task: impl FnMut(usize) + 'static,
  interval_duration: Duration,
  delay: Duration,
) -> impl Future<Output = ()> {
  let mut number = 0;

  futures::future::ready(())
    .then(move |_| {
      task(number);
      // futures::future::ready(()).then(move |_| {
      //   // futures::future::ready(()).delay(interval_duration)
      //   task(number+1);
      //   futures::future::ready(())
      // }).delay(interval_duration)
      futures::future::ready(()).delay(delay)
    })
    .delay(delay)
}

#[cfg(feature = "tokio-scheduler")]
mod tokio_scheduler {
  use super::*;
  use std::sync::Arc;
  use tokio::runtime::Runtime;

  impl SharedScheduler for Runtime {
    fn spawn<Fut>(&self, future: Fut)
    where
      Fut: Future<Output = ()> + Send + 'static,
    {
      Runtime::spawn(self, future);
    }
  }

  impl SharedScheduler for Arc<Runtime> {
    fn spawn<Fut>(&self, future: Fut)
    where
      Fut: Future<Output = ()> + Send + 'static,
    {
      Runtime::spawn(self, future);
    }
  }
}

#[cfg(all(test, feature = "tokio-scheduler"))]
mod test {
  use crate::prelude::*;
  use bencher::Bencher;
  use futures::executor::{LocalPool, ThreadPool};
  use std::sync::{Arc, Mutex};

  fn waste_time(v: u32) -> u32 {
    (0..v)
      .into_iter()
      .map(|index| (0..index).sum::<u32>().min(u32::MAX / v))
      .sum()
  }

  #[test]
  fn bench_pool() { do_bench_pool(); }

  benchmark_group!(do_bench_pool, pool);

  fn pool(b: &mut Bencher) {
    let last = Arc::new(Mutex::new(0));
    b.iter(|| {
      let c_last = last.clone();
      let pool = ThreadPool::new().unwrap();
      observable::from_iter(0..1000)
        .observe_on(pool)
        .map(waste_time)
        .into_shared()
        .subscribe(move |v| *c_last.lock().unwrap() = v);

      *last.lock().unwrap()
    })
  }

  #[test]
  fn bench_local_thread() { do_bench_local_thread(); }

  benchmark_group!(do_bench_local_thread, local_thread);

  fn local_thread(b: &mut Bencher) {
    let last = Arc::new(Mutex::new(0));
    b.iter(|| {
      let c_last = last.clone();
      let mut local = LocalPool::new();
      observable::from_iter(0..1000)
        .observe_on(local.spawner())
        .map(waste_time)
        .subscribe(move |v| *c_last.lock().unwrap() = v);
      local.run();
      *last.lock().unwrap()
    })
  }

  #[test]
  fn bench_tokio_basic() { do_bench_tokio_basic(); }

  benchmark_group!(do_bench_tokio_basic, tokio_basic);

  fn tokio_basic(b: &mut Bencher) {
    use tokio::runtime;
    let last = Arc::new(Mutex::new(0));
    b.iter(|| {
      let c_last = last.clone();
      let local = runtime::Builder::new_current_thread().build().unwrap();

      observable::from_iter(0..1000)
        .observe_on(local)
        .map(waste_time)
        .into_shared()
        .subscribe(move |v| *c_last.lock().unwrap() = v);

      *last.lock().unwrap()
    })
  }

  #[test]
  fn bench_tokio_thread() { do_bench_tokio_thread(); }

  benchmark_group!(do_bench_tokio_thread, tokio_thread);

  fn tokio_thread(b: &mut Bencher) {
    use tokio::runtime;
    let last = Arc::new(Mutex::new(0));
    b.iter(|| {
      let c_last = last.clone();
      let pool = runtime::Runtime::new().unwrap();
      observable::from_iter(0..1000)
        .observe_on(pool)
        .map(waste_time)
        .into_shared()
        .subscribe(move |v| *c_last.lock().unwrap() = v);

      // todo: no way to wait all task has finished in `Tokio` Scheduler.

      *last.lock().unwrap()
    })
  }
}
