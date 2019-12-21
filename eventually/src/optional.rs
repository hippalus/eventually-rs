//! Support for [`Aggregate`] with [`Option`] state.
//!
//! [`Aggregate`]: ../aggregate/trait.Aggregate.html
//! [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html

use async_trait::async_trait;

use crate::{aggregate, command};

/// _Command Handler_ trait referring to [`Aggregate`] with [`Option`] state,
/// a.k.a. [`Aggregate`].
///
/// Implementations of this trait can be adapted back into the [`command::Handler`]
/// foundation trait by using [`as_handler`], in cases where the implementation
/// has compile-time known size.
///
/// [`Aggregate`]: ../aggregate/trait.Aggregate.html
/// [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
/// [`Aggregate`]: trait.Aggregate.html
/// [`command::Handler`]: ../command/trait.Handler.html
/// [`as_handler`]: trait.CommandHandler.html#method.as_handler
#[async_trait]
pub trait CommandHandler {
    /// Commands to trigger a specific use-case on the context of an [`Aggregate`].
    ///
    /// Most often than not, this type should be an `enum` containing
    /// all supported operations -- that are not queries -- for the specified [`Aggregate`].
    ///
    /// [`Aggregate`]: trait.CommandHandler.html#associatedType.Aggregate
    type Command;

    /// _Domain entity_ produced, updated or, in some way, affected by a [`Command`].
    ///
    /// [`Command`]: trait.CommandHandler.html#associatedType.Command
    type Aggregate: Aggregate;

    /// Possible expected errors to be returned when handling a [`Command`] fails.
    ///
    /// [`Command`]: trait.CommandHandler.html#associatedType.Command
    type Error;

    /// Handles a [`Command`] when the [`Aggregate`] state is not yet present.
    ///
    /// Usually this happens when the event store has no persisted event
    /// for this aggregate yet.
    ///
    /// [`Command`]: trait.CommandHandler.html#associatedType.Command
    /// [`Aggregate`]: trait.CommandHandler.html#associatedType.Aggregate
    async fn handle_first(
        &self,
        command: Self::Command,
    ) -> command::Result<EventOf<Self::Aggregate>, Self::Error>;

    /// Handles a [`Command`] when the previous [`Aggregate`] state
    /// is already **present** and **available** to the command handler.
    ///
    /// [`Command`]: trait.CommandHandler.html#associatedType.Command
    /// [`Aggregate`]: trait.CommandHandler.html#associatedType.Aggregate
    async fn handle_next(
        &self,
        state: &StateOf<Self::Aggregate>,
        command: Self::Command,
    ) -> command::Result<EventOf<Self::Aggregate>, Self::Error>;

    /// Adapts the [`CommandHandler`] implementation to the [`command::Handler`]
    /// foundation trait, useful when needs to be used with a
    /// [`command::Dispatcher`].
    ///
    /// This method is only available when the `Self` has
    /// compile-time known size.
    ///
    /// [`CommandHandler`]: trait.CommandHandler.html
    /// [`command::Handler`]: ../command/trait.Handler.html
    /// [`command::Dispatcher`]: ../command/dispatcher/struct.Dispatcher.html
    fn as_handler(self) -> AsHandler<Self>
    where
        Self: Sized,
    {
        AsHandler(self)
    }
}

/// Adapter for [`CommandHandler`] implementators to [`command::Handler`] trait.
///
/// Use [`CommandHandler.as_handler`] to construct this object.
///
/// [`CommandHandler`]: trait.CommandHandler.html
/// [`command::Handler`]: ../command/trait.Handler.html
/// [`CommandHandler.as_handler`]: trait.CommandHandler.html#method.as_handler
pub struct AsHandler<H>(H);

#[async_trait]
impl<H> command::Handler for AsHandler<H>
where
    H: CommandHandler + Send + Sync,
    StateOf<H::Aggregate>: Send + Sync,
    H::Command: Send,
{
    type Command = H::Command;
    type Aggregate = AsAggregate<H::Aggregate>;
    type Error = H::Error;

    async fn handle(
        &self,
        state: &aggregate::StateOf<Self::Aggregate>,
        command: Self::Command,
    ) -> command::Result<aggregate::EventOf<Self::Aggregate>, Self::Error> {
        match state {
            None => self.0.handle_first(command),
            Some(state) => self.0.handle_next(state, command),
        }
        .await
    }
}

/// Extract the [`State`] from an [`Aggregate`].
///
/// [`Aggregate`]: trait.Aggregate.html
/// [`State`]: trait.Aggregate.html#associatedType.State
pub type StateOf<A: Aggregate> = A::State;

/// Extract the [`Event`] from an [`Aggregate`].
///
/// [`Aggregate`]: trait.Aggregate.html
/// [`Event`]: trait.Aggregate.html#associatedType.Event
pub type EventOf<A: Aggregate> = A::Event;

/// Variation of [`aggregate::Aggregate`] trait, useful when
/// the Aggregate [`State`] is expressed as an [`Option`].
///
/// Implementors of this trait can be adapted to the foundation [`aggregate::Aggregate`]
/// trait by using the [`AsAggregate`] adapter.
///
/// [`aggregate::Aggregate`]: ../aggregate/trait.Aggregate.html
/// [`State`]: ../aggregate/trait.Aggregate.html#associatedType.State
/// [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
/// [`AsAggregate`]: struct.AsAggregate.html
pub trait Aggregate {
    /// State of the Aggregate.
    ///
    /// **DO NOT** use an [`Option`] here: this type is thought
    /// to be as the `T` type in `Option<T>`.
    ///
    /// [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
    type State;

    /// Event of the Aggregate.
    ///
    /// Check out [`Event`] documentation for more information.
    ///
    /// [`Event`]: ../aggregate/trait.Aggregate.html#associatedType.Event
    type Event;

    /// Error occurring when appling an [`Event`] to an Aggregate.
    ///
    /// Check out [`Error`] documentation for more information.
    ///
    /// [`Event`]: trait.Aggregate.html#associatedType.Event
    /// [`Error`]: ../aggregate/trait.Aggregate.html#associatedType.Error
    type Error;

    /// Handles events when the [`State`] has not been found.
    ///
    /// [`State`]: trait.Aggregate.html#associatedType.State
    fn apply_first(event: Self::Event) -> Result<Self::State, Self::Error>;

    /// Handles events when the [`State`] has been found,
    /// and updates it accordingly.
    ///
    /// [`State`]: trait.Aggregate.html#associatedType.State
    fn apply_next(state: Self::State, event: Self::Event) -> Result<Self::State, Self::Error>;
}

/// Adapter for [`Aggregate`] types to the foundational [`eventually::Aggregate`] trait.
///
/// # Examples
///
/// ```
/// use eventually::optional::Aggregate as OptionalAggregate;
///
/// enum SomeEvent {
///     Happened
/// }
///
/// #[derive(Debug, PartialEq)]
/// struct SomeState {
///     // Some nice fields
/// }
///
/// struct SomeAggregate;
/// impl OptionalAggregate for SomeAggregate {
///     type State = SomeState;
///     type Event = SomeEvent;
///     type Error = std::convert::Infallible;
///
///     fn apply_first(event: Self::Event) -> Result<Self::State, Self::Error> {
///         // Return an empty state, here you should create the initial
///         // state based on the event received.
///         Ok(SomeState {})
///     }
///
///     fn apply_next(state: Self::State, _event: Self::Event) -> Result<Self::State, Self::Error> {
///         // Return the same state, here you should update the state
///         // based on the event received.
///         Ok(state)
///     }
/// }
///
/// use eventually::Aggregate;
/// use eventually::optional::AsAggregate;
///
/// // To adapt SomeAggregate to `eventually::Aggregate`:
/// let result = AsAggregate::<SomeAggregate>::apply(
///     None,                   // This state will result in calling `SomeAggregate::apply_first`
///     SomeEvent::Happened,
/// );
///
/// // An `Option`-wrapped `SomeState` instance is returned.
/// assert_eq!(result, Ok(Some(SomeState {})));
/// ```
///
/// [`Aggregate`]: trait.Aggregate.html
/// [`eventually::Aggregate`]: ../aggregate/trait.Aggregate.html
pub struct AsAggregate<T>(std::marker::PhantomData<T>);

impl<A> aggregate::Aggregate for AsAggregate<A>
where
    A: Aggregate,
{
    type State = Option<A::State>;
    type Event = A::Event;
    type Error = A::Error;

    fn apply(state: Self::State, event: Self::Event) -> Result<Self::State, Self::Error> {
        Ok(Some(match state {
            None => A::apply_first(event)?,
            Some(state) => A::apply_next(state, event)?,
        }))
    }
}