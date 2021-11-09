use crate::prelude::*;

//todo use atomic bool replace Box<dyn Publisher<Item = Item, Err = Err> + Send
// + Sync>
pub type SharedSubject<Item, Err> = MutArc<
  Subject<
    Box<dyn Observer<Item = Item, Err = Err> + Send + Sync>,
    MutArc<SingleSubscription>,
  >,
>;

pub type SharedBehaviorSubject<Item, Err> = MutArc<
  BehaviorSubject<
    Box<dyn Observer<Item = Item, Err = Err> + Send + Sync>,
    MutArc<SingleSubscription>,
  >,
>;

impl<Item, Err> SharedSubject<Item, Err> {
  #[inline]
  pub fn new() -> Self { Self::default() }
}
impl<Item, Err> Observable for SharedSubject<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<Item, Err> SharedObservable for SharedSubject<Item, Err> {
  type Unsub = MutArc<SingleSubscription>;
  fn actual_subscribe<O>(self, observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    self.rc_deref_mut().subscribe(Box::new(observer))
  }
}

impl<Item, Err> Observable for SharedBehaviorSubject<Item, Err> {
  type Item = Item;
  type Err = Err;
}

impl<Item, Err> SharedObservable for SharedBehaviorSubject<Item, Err>
where
  Item: Clone,
  Err: Clone,
{
  type Unsub = MutArc<SingleSubscription>;
  fn actual_subscribe<O>(self, mut observer: O) -> Self::Unsub
  where
    O: Observer<Item = Self::Item, Err = Self::Err> + Sync + Send + 'static,
  {
    if !self.rc_deref().subject.is_closed() {
      observer.next(self.rc_deref().value.clone());
    }
    self.rc_deref_mut().subject.subscribe(Box::new(observer))
  }
}

impl<Item, Err> SharedBehaviorSubject<Item, Err> {
  #[inline]
  pub fn new(value: Item) -> Self { MutArc::own(BehaviorSubject::new(value)) }
}

#[test]
fn smoke() {
  let test_code = MutArc::own("".to_owned());
  let mut subject = SharedSubject::new();
  let c_test_code = test_code.clone();
  subject.clone().into_shared().subscribe(move |v: &str| {
    *c_test_code.rc_deref_mut() = v.to_owned();
  });
  subject.next("test shared subject");
  assert_eq!(*test_code.rc_deref_mut(), "test shared subject");
  assert_eq!(subject.teardown_size(), 1);
}
