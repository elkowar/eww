use gtk::glib::{self, object_subclass, wrapper, Properties};
use gtk::{prelude::*, subclass::prelude::*};
use std::cell::RefCell;

wrapper! {
    pub struct Window(ObjectSubclass<WindowPriv>)
    @extends gtk::Window, gtk::Bin, gtk::Container, gtk::Widget, @implements gtk::Buildable;
}

#[derive(Properties)]
#[properties(wrapper_type = Window)]
pub struct WindowPriv {
    #[property(get, name = "x", nick = "X", blurb = "Global x coordinate", default = 0)]
    x: RefCell<i32>,

    #[property(get, name = "y", nick = "Y", blurb = "Global y coordinate", default = 0)]
    y: RefCell<i32>,
}

// This should match the default values from the ParamSpecs
impl Default for WindowPriv {
    fn default() -> Self {
        WindowPriv { x: RefCell::new(0), y: RefCell::new(0) }
    }
}

#[object_subclass]
impl ObjectSubclass for WindowPriv {
    type ParentType = gtk::Window;
    type Type = Window;

    const NAME: &'static str = "WindowEww";
}

impl Default for Window {
    fn default() -> Self {
        glib::Object::new::<Self>()
    }
}

impl Window {
    pub fn new(type_: gtk::WindowType, x_: i32, y_: i32) -> Self {
        let w: Self = glib::Object::builder().property("type", type_).build();
        let priv_ = w.imp();
        priv_.x.replace(x_);
        priv_.y.replace(y_);
        w
    }
}

impl ObjectImpl for WindowPriv {
    fn properties() -> &'static [glib::ParamSpec] {
        Self::derived_properties()
    }

    fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        self.derived_property(id, pspec)
    }
}
impl WindowImpl for WindowPriv {}
impl BinImpl for WindowPriv {}
impl ContainerImpl for WindowPriv {}
impl WidgetImpl for WindowPriv {}
