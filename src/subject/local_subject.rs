use crate::prelude::*;

pub type _LocalSubject<O> = MutRc<Subject<O, MutRc<SingleSubscription>>>;
pub type _LocalBehaviorSubject<O> =
  MutRc<BehaviorSubject<O, MutRc<SingleSubscription>>>;

pub type LocalSubject<'a, Item, Err> =
  _LocalSubject<Box<dyn Observer<Item = Item, Err = Err> + 'a>>;

pub type LocalSubjectRef<'a, Item, Err> =
  _LocalSubject<Box<dyn Observer<Item = &'a Item, Err = Err> + 'a>>;

pub type LocalSubjectErrRef<'a, Item, Err> =
  _LocalSubject<Box<dyn Observer<Item = Item, Err = &'a Err> + 'a>>;

pub type LocalSubjectRefAll<'a, Item, Err> =
  _LocalSubject<Box<dyn Observer<Item = &'a Item, Err = &'a Err> + 'a>>;

pub type LocalBehaviorSubject<'a, Item, Err> =
  _LocalBehaviorSubject<Box<dyn Observer<Item = Item, Err = Err> + 'a>>;

pub type LocalBehaviorSubjectRef<'a, Item, Err> =
  _LocalBehaviorSubject<Box<dyn Observer<Item = &'a Item, Err = Err> + 'a>>;

pub type LocalBehaviorSubjectErrRef<'a, Item, Err> =
  _LocalBehaviorSubject<Box<dyn Observer<Item = Item, Err = &'a Err> + 'a>>;

pub type LocalBehaviorSubjectRefAll<'a, Item, Err> =
  _LocalBehaviorSubject<Box<dyn Observer<Item = &'a Item, Err = &'a Err> + 'a>>;

impl<'a, Item, Err> Observable for LocalSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item, Err> LocalObservable<'a> for LocalSubject<'a, Item, Err> {
  type Unsub = MutRc<SingleSubscription>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    self.rc_deref_mut().subscribe(Box::new(observer))
  }
}

impl<'a, Item, Err> Observable for LocalBehaviorSubject<'a, Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<'a, Item: Clone, Err> LocalObservable<'a>
  for LocalBehaviorSubject<'a, Item, Err>
{
  type Unsub = MutRc<SingleSubscription>;
  fn actual_subscribe<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + 'a,
  {
    if !self.rc_deref().subject.is_closed() {
      observer.next(self.rc_deref().value.clone());
    }
    self.rc_deref_mut().subject.subscribe(Box::new(observer))
  }
}

impl<O: Observer> _LocalSubject<O> {
  #[inline]
  pub fn new() -> Self { Self::default() }
}

impl<O: Observer> _LocalBehaviorSubject<O> {
  #[inline]
  pub fn new(value: O::Item) -> Self { MutRc::own(BehaviorSubject::new(value)) }
}

#[cfg(test)]
mod test {
  use crate::prelude::*;

  #[test]
  fn smoke() {
    let mut test_code = 1;
    {
      let mut subject = LocalSubject::default();
      subject.clone().subscribe(|v| {
        test_code = v;
      });
      subject.next(2);

      assert_eq!(subject.teardown_size(), 1);
    }
    assert_eq!(test_code, 2);
  }

  #[test]
  fn emit_ref() {
    let mut check = 0;

    {
      let mut subject = LocalSubjectRef::new();
      subject.clone().subscribe(|v| {
        check = *v;
      });
      subject.next(&1);
    }
    assert_eq!(check, 1);

    {
      let mut subject = LocalSubjectErrRef::new();
      subject
        .clone()
        .subscribe_err(|_: ()| {}, |err| check = *err);
      subject.error(&2);
    }
    assert_eq!(check, 2);

    {
      let mut subject = LocalSubjectRefAll::new();
      subject.clone().subscribe_err(|v| check = *v, |_: &()| {});
      subject.next(&1);
    }
    assert_eq!(check, 1);

    {
      let mut subject = LocalSubjectRefAll::new();
      subject
        .clone()
        .subscribe_err(|_: &()| {}, |err| check = *err);
      subject.error(&2);
    }
    assert_eq!(check, 2);
  }
}
