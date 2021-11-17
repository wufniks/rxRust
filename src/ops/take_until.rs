use crate::{impl_helper::*, impl_local_share_both, prelude::*};

#[derive(Clone)]
pub struct TakeUntilOp<S, N> {
  pub(crate) source: S,
  pub(crate) notifier: N,
}

impl<S: Observable, N> Observable for TakeUntilOp<S, N> {
  type Item = S::Item;
  type Err = S::Err;
}

impl_local_share_both! {
  impl<S, N> TakeUntilOp<S, N>;
  type Unsub = @ctx::RcMultiSubscription;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    let subscription = $ctx::RcMultiSubscription::default();
    // We need to keep a reference to the observer from two places
    let shared_observer = $ctx::Rc::own($observer);

    subscription.add($self.notifier.actual_subscribe(
      TakeUntilNotifierObserver {
        subscription: subscription.clone(),
        main_observer: shared_observer.clone(),
        _p: TypeHint::new(),
      },
    ));
    subscription.add($self.source.actual_subscribe(shared_observer));
    subscription
  }
  where
    S: @ctx::Observable,
    N: @ctx::Observable<Err=S::Err>  @ctx::local_only(+ 'o),
    @ctx::shared_only(N::Item: 'static,)
    S::Unsub: 'static,
    N::Unsub: 'static


}

pub struct TakeUntilNotifierObserver<O, U, Item> {
  // We need access to main observer in order to call `complete` on it as soon
  // as notifier fired
  main_observer: O,
  // We need to unsubscribe everything as soon as notifier fired
  subscription: U,
  _p: TypeHint<Item>,
}

impl<O, U, NotifierItem, Err> Observer
  for TakeUntilNotifierObserver<O, U, NotifierItem>
where
  O: Observer<Err = Err>,
  U: SubscriptionLike,
{
  type Item = NotifierItem;
  type Err = Err;
  fn next(&mut self, _: NotifierItem) {
    self.main_observer.complete();
    self.subscription.unsubscribe();
  }

  fn error(&mut self, err: Err) {
    self.main_observer.error(err);
    self.subscription.unsubscribe();
  }

  #[inline]
  fn complete(&mut self) { self.subscription.unsubscribe() }
}

#[cfg(test)]
mod test {
  use std::sync::{Arc, Mutex};

  use crate::prelude::*;

  #[test]
  fn base_function() {
    let mut last_next_arg = None;
    let mut next_count = 0;
    let mut completed_count = 0;
    {
      let mut notifier = LocalSubject::new();
      let mut source = LocalSubject::new();
      source
        .clone()
        .take_until(notifier.clone())
        .subscribe_complete(
          |i| {
            last_next_arg = Some(i);
            next_count += 1;
          },
          || {
            completed_count += 1;
          },
        );
      source.next(5);
      notifier.next(());
      source.next(6);
      notifier.complete();
      source.complete();
    }
    assert_eq!(next_count, 1);
    assert_eq!(last_next_arg, Some(5));
    assert_eq!(completed_count, 1);
  }

  #[test]
  fn ininto_shared() {
    let last_next_arg = Arc::new(Mutex::new(None));
    let last_next_arg_mirror = last_next_arg.clone();
    let next_count = Arc::new(Mutex::new(0));
    let next_count_mirror = next_count.clone();
    let completed_count = Arc::new(Mutex::new(0));
    let completed_count_mirror = completed_count.clone();
    let mut notifier = SharedSubject::new();
    let mut source = SharedSubject::new();
    source
      .clone()
      .take_until(notifier.clone())
      .into_shared()
      .subscribe_complete(
        move |i| {
          *last_next_arg.lock().unwrap() = Some(i);
          *next_count.lock().unwrap() += 1;
        },
        move || {
          *completed_count.lock().unwrap() += 1;
        },
      );
    source.next(5);
    notifier.next(());
    source.next(6);
    assert_eq!(*next_count_mirror.lock().unwrap(), 1);
    assert_eq!(*last_next_arg_mirror.lock().unwrap(), Some(5));
    assert_eq!(*completed_count_mirror.lock().unwrap(), 1);
  }

  #[test]
  fn bench() { do_bench(); }

  benchmark_group!(do_bench, bench_take_until);

  fn bench_take_until(b: &mut bencher::Bencher) { b.iter(base_function); }
}
