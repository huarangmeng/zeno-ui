use std::{
    any::Any,
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
};

use crate::ActionId;

type BoxedMessage = Box<dyn Any>;

enum MessageBinding {
    Click(Box<dyn Fn() -> BoxedMessage>),
    Toggle(Box<dyn Fn(bool) -> BoxedMessage>),
}

#[derive(Default)]
struct MessageBindingBuilder {
    next_action_id: u64,
    bindings: HashMap<ActionId, MessageBinding>,
}

#[derive(Default)]
pub struct MessageBindings {
    bindings: HashMap<ActionId, MessageBinding>,
}

thread_local! {
    static ACTIVE_MESSAGE_BINDINGS: RefCell<Option<MessageBindingBuilder>> = const { RefCell::new(None) };
}

static FALLBACK_ACTION_ID: AtomicU64 = AtomicU64::new(0x7000_0000);

impl MessageBindingBuilder {
    fn next_action_id(&mut self) -> ActionId {
        let id = ActionId(0x4000_0000 + self.next_action_id);
        self.next_action_id += 1;
        id
    }
}

#[doc(hidden)]
pub fn begin_message_bindings() {
    ACTIVE_MESSAGE_BINDINGS.with(|cell| {
        *cell.borrow_mut() = Some(MessageBindingBuilder::default());
    });
}

#[doc(hidden)]
pub fn finish_message_bindings() -> MessageBindings {
    ACTIVE_MESSAGE_BINDINGS.with(|cell| {
        let builder = cell.borrow_mut().take().unwrap_or_default();
        MessageBindings {
            bindings: builder.bindings,
        }
    })
}

#[doc(hidden)]
pub fn bind_click_message<M>(message: M) -> ActionId
where
    M: Clone + 'static,
{
    let message = Arc::new(message);
    ACTIVE_MESSAGE_BINDINGS.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let Some(builder) = borrow.as_mut() else {
            return ActionId(FALLBACK_ACTION_ID.fetch_add(1, Ordering::Relaxed));
        };
        let action_id = builder.next_action_id();
        builder.bindings.insert(
            action_id,
            MessageBinding::Click(Box::new(move || Box::new((*message).clone()))),
        );
        action_id
    })
}

#[doc(hidden)]
pub fn bind_toggle_message<M, F>(mapper: F) -> ActionId
where
    M: 'static,
    F: Fn(bool) -> M + 'static,
{
    ACTIVE_MESSAGE_BINDINGS.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let Some(builder) = borrow.as_mut() else {
            return ActionId(FALLBACK_ACTION_ID.fetch_add(1, Ordering::Relaxed));
        };
        let action_id = builder.next_action_id();
        builder.bindings.insert(
            action_id,
            MessageBinding::Toggle(Box::new(move |checked| Box::new(mapper(checked)))),
        );
        action_id
    })
}

impl MessageBindings {
    pub fn resolve_click<M: 'static>(&self, action_id: ActionId) -> Option<M> {
        let binding = self.bindings.get(&action_id)?;
        match binding {
            MessageBinding::Click(mapper) => mapper().downcast::<M>().ok().map(|boxed| *boxed),
            MessageBinding::Toggle(_) => None,
        }
    }

    pub fn resolve_toggle<M: 'static>(&self, action_id: ActionId, checked: bool) -> Option<M> {
        let binding = self.bindings.get(&action_id)?;
        match binding {
            MessageBinding::Toggle(mapper) => {
                mapper(checked).downcast::<M>().ok().map(|boxed| *boxed)
            }
            MessageBinding::Click(_) => None,
        }
    }
}
