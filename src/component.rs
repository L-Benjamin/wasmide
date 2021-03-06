//! Reusable components for the application.
//! 
//! This module exposes the type [`Component`] which is a wrapper around an
//! HTML element. It can be used to create reusable components.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering::SeqCst};

use alloc::boxed::Box;
use alloc::rc::{Rc, Weak};
use alloc::string::ToString;
use alloc::vec::Vec;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::HtmlElement;

use crate::prelude::*;
use crate::store::StoreUnsubscriber;

// A depencency of the component, that needs to be dropped at the same
// time as the component.
enum Dependency {
    Children(Component),
    Closure(Closure<dyn FnMut()>),
    Subscription(StoreUnsubscriber),
}

impl Drop for Dependency {
    // On drop, unsubscription needs to be applied.
    #[inline]
    fn drop(&mut self) {
        match self {
            Self::Subscription(subscription) => subscription.unsubscribe(),
            _ => (),
        }
    }
}

// The internal state of a component.
struct InternalComponent {
    element: HtmlElement,
    deps: UnsafeCell<Vec<Dependency>>,
}

impl InternalComponent {
    // Creates a new component with the given html element.
    #[inline]
    fn new(element: HtmlElement) -> Self {
        Self {
            element,
            deps: Vec::new().into(),
        }
    }
}

/// Reusable component, corresponds to a single html element of the page.
/// 
/// # Examples
/// 
/// ```no_run
/// # use wasmide::prelude::*;
/// let root = Component::root(Style::NONE);
/// ```
#[derive(Clone)]
pub struct Component(Rc<InternalComponent>);

impl Component {
    // Push a dependency to the component's storage.
    #[inline]
    fn push_dep(&self, dep: Dependency) {
        // SAFETY: The deps vec is never borrowed.
        unsafe { (*self.0.deps.get()).push(dep) };
    }

    // Appends a child to the component.
    #[inline]
    fn append_child(&self, child: Component) {
        self.html().append_child(child.html()).unwrap();
        self.push_dep(Dependency::Children(child));
    }

    // Push a closure to the component's storage.
    #[inline]
    fn push_closure(&self, closure: Closure<dyn FnMut()>) {
        self.push_dep(Dependency::Closure(closure));
    }

    // Adds an unubsription to the component, to be performed when it is dropped.
    #[inline]
    fn push_unsub(&self, unsub: StoreUnsubscriber) {
        self.push_dep(Dependency::Subscription(unsub));
    }

    // Sets the inner html attribute of the element on store update.
    #[inline]
    pub(crate) fn set_inner_html<S: ToString>(&self, text: impl Subscribable<S>) {
        let weak = self.downgrade();

        let unsub = text.subscribe(move |text| {
            if let Some(comp) = weak.upgrade() {
                comp.html().set_inner_html(&text.to_string());
            }
        });

        self.push_unsub(unsub);
    }

    #[inline]
    pub(crate) fn set_on_click(&self, on_click: impl FnMut() + 'static) {
        let on_click = Closure::wrap(Box::new(on_click) as Box<dyn FnMut()>);
        self.html().set_onclick(Some(on_click.as_ref().unchecked_ref()));
        self.push_closure(on_click);
    }

    // Creates a new component with the given html tag_name and style.
    #[inline]
    pub(crate) fn new(tag_name: &'static str, style: Style) -> Self {
        let element = web_sys::window().unwrap()
            .document().unwrap()
            .create_element(tag_name).unwrap()
            .dyn_into().unwrap();

        let this = Component(Rc::new(InternalComponent::new(element)));
        if style != Style::NONE {
            this.set_style(style);
        }
        this
    }

    /// Returns the root component of the application. The returned component will
    /// never get dropped. May only be called once.
    /// 
    /// # Panics
    /// 
    /// Panics if called more than once.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// # use wasmide::prelude::*;
    /// Component::root(Style::NONE)
    ///     .with(html::p(Value("Hello, world!"), Style::NONE)); 
    /// ```
    #[inline]
    pub fn root(style: Style) -> Component {
        // For data races.
        static INITIALIZED: AtomicBool = AtomicBool::new(false);
        // To prevent the body from being dropped.
        static mut ROOT: Option<Component> = None;

        INITIALIZED.compare_exchange(false, true, SeqCst, SeqCst).expect("root already initialized");
        
        let body = web_sys::window().unwrap()
            .document().unwrap()
            .body().unwrap();

        let comp = Component(Rc::new(InternalComponent::new(body)));
        comp.set_style(style);

        // SAFETY: Thread-safe thanks to the INITIALIZED atomic flag.
        unsafe { ROOT = Some(comp.clone()); }

        comp
    }

    /// Returns a reference to the html element wrapped by the component.
    /// This allows direct modification of the html but it is discouraged as
    /// it can lead to unexpected behavior. 
    /// 
    /// For example, a children of a component, appended with [`Component::with_if`]
    /// will get hidden if the condition is false. Manually making it visible will
    /// cause problems.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// # use wasmide::prelude::*;
    /// let root = Component::root(Style::NONE);
    /// let html = root.html();
    /// ```
    #[inline]
    pub fn html(&self) -> &HtmlElement {
        &self.0.element
    }

    /// Creates a new [`WeakComponent`] reference to this component.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// # use wasmide::prelude::*;
    /// let root = Component::root(Style::NONE);
    /// let weak = root.downgrade();
    /// ```
    #[inline]
    pub fn downgrade(&self) -> WeakComponent {
        WeakComponent(Rc::downgrade(&self.0))
    }

    /// Sets the style of the component.
    /// 
    /// This will in fact only set the class attribute of the html element
    /// to the string wrapped in the given style.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// # use wasmide::prelude::*;
    /// let root = Component::root(Style::NONE);
    /// root.set_style(Style("bg-blue-200"));
    /// ```
    #[inline]
    pub fn set_style(&self, style: Style) {
        self.html().set_class_name(style.0);
    }

    /// Appends a child to the component. This method is meant to be chained.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// # use wasmide::prelude::*;
    /// Component::root(Style::NONE)
    ///     .with(html::p(Value("Hello, world!"), Style::NONE));
    /// ```
    #[inline]
    pub fn with(self, component: Component) -> Self {
        self.append_child(component);
        self
    }

    /// Appends a child to the component if the condition is true.
    /// This method is meant to be chained.
    /// 
    /// When the condition becomes false, the child will be hidden, and then shown 
    /// again when it becomes true.
    /// If the consition is initially false, the child will be lazyly initialized when
    /// it becomes true.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// # use wasmide::prelude::*;
    /// let store = Store::new(42);
    /// 
    /// Component::root(Style::NONE)
    ///    .with_if(
    ///         store.compose(|n| *n > 42),
    ///         || html::p(Value("Greater than 42"), Style::NONE),
    ///   );
    /// ``` 
    #[inline]
    pub fn with_if(
        self, 
        condition: impl Subscribable<bool>, 
        child: impl Fn() -> Component + 'static,
    ) -> Self {
        let this = self.downgrade();
        let mut comp = Children::new(child);

        let unsub = condition.subscribe(move |&condition| {
            if condition {
                comp.activate(&this);
            } else {
                comp.deactivate();
            }
        });

        self.push_unsub(unsub);
        self
    }

    /// Appends a child to the component if the condition is true, else
    /// appends another child.
    /// This method is meant to be chained.
    /// 
    /// When the condition becomes false, the first child will be hidden and the second
    /// shown. When it becomes true, the second child will be hidden and the first shown.
    /// 
    /// If the condition is initially false, the first child will be lazyly initialized when
    /// it becomes true. If it is true initially, the second child will be lazyly initialized
    /// when it becomes false.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// # use wasmide::prelude::*;
    /// let hour = Store::new(6);
    /// 
    /// Component::root(Style::NONE)
    ///    .with_if_else(
    ///         hour.compose(|h| *h > 9 && *h < 18),
    ///         || html::p(Value("Hello, world!"), Style::NONE),
    ///         || html::p(Value("Goodbye, world!"), Style::NONE),
    ///   );
    /// ``` 
    #[inline]
    pub fn with_if_else(
        self, 
        condition: impl Subscribable<bool>, 
        child1: impl Fn() -> Component + 'static, 
        child2: impl Fn() -> Component + 'static,
    ) -> Self {
        let this = self.downgrade();
        let mut comp1 = Children::new(child1);
        let mut comp2 = Children::new(child2);

        let unsub = condition.subscribe(move |&condition| {
            if condition {
                comp1.activate(&this);
                comp2.deactivate();
            } else {
                comp1.deactivate();
                comp2.activate(&this);
            }
        });

        self.push_unsub(unsub);
        self
    }
}

// An enum representing a lazy-initialized component, that is to be 
// attached to a parent when crated, and hidden when deactivated.
enum Children<F: FnOnce() -> Component> {
    Uninit(Option<F>),
    Init(Component),
}

impl<F: FnOnce() -> Component> Children<F> {
    // Creates a new childen from the given function.
    #[inline]
    fn new(init: F) -> Self {
        Self::Uninit(Some(init))
    }

    // Creates a new component and attaches it to the given parent
    // if it is not already initialized, else set it to not hidden.
    #[inline]
    fn activate(&mut self, parent: &WeakComponent) {
        match self {
            Self::Uninit(init) => {
                let comp = init.take().unwrap()();
                parent.upgrade().unwrap().append_child(comp.clone());
                *self = Self::Init(comp);
            },
            Self::Init(ref comp) => {
                comp.html().set_hidden(false);
            }
        }
    }

    // If the component is initialized, set it to hidden.
    #[inline]
    fn deactivate(&self) {
        if let Self::Init(comp) = self {
            comp.html().set_hidden(true);
        }
    }
}

/// Weak reference to a component.
/// 
/// Does not prevent the component it refers to from being dropped.
/// 
/// # Examples
/// 
/// ```no_run
/// # use wasmide::prelude::*;
/// let body = Component::root(Style::NONE);
/// let weak = body.downgrade();
/// ```
#[derive(Clone)]
pub struct WeakComponent(Weak<InternalComponent>);

impl WeakComponent {
    /// Tries to get a strong reference to the component. Will return `None` if the component
    /// has been dropped, or `Some(Component)` if it was not.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// # use wasmide::prelude::*;
    /// let body = Component::root(Style::NONE);
    /// let weak = body.downgrade();
    /// if let Some(body) = weak.upgrade() {
    ///     // do stuff...
    /// }
    /// ```
    #[inline]
    pub fn upgrade(&self) -> Option<Component> {
        self.0.upgrade().map(Component)
    }
}