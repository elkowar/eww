use std::cell::RefCell;
// https://www.figuiere.net/technotes/notes/tn002/
// https://github.com/gtk-rs/examples/blob/master/src/bin/listbox_model.rs
use anyhow::Result;
use glib::{object_subclass, wrapper};
use gtk::{prelude::*, subclass::prelude::*};

use crate::error_handling_ctx;

wrapper! {
    pub struct Graph(ObjectSubclass<GraphPriv>)
    @extends gtk::Bin, gtk::Container, gtk::Widget;
}

// wrapper! { pub struct Graph(ObjectSubclass<GraphPriv>) @extends gtk::Box, gtk::Container, gtk::Widget; }

#[derive(Default)]
pub struct GraphPriv {
    value: RefCell<f32>,
    content: RefCell<Option<gtk::Widget>>,
}

impl ObjectImpl for GraphPriv {
    // glib_object_impl!();
    fn properties() -> &'static [glib::ParamSpec] {
        use once_cell::sync::Lazy;
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpec::new_float("value", "Value", "The value", 0f32, 100f32, 0f32, glib::ParamFlags::READWRITE),
            ]
        });

        PROPERTIES.as_ref()
    }

    //    fn constructed(&self, obj: &Self::Type) {
    //        self.parent_constructed(obj);
    //    }

    fn set_property(&self, _obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "value" => {
                self.value.replace(value.get().unwrap());
            }

            x => panic!("Tried to set inexistant property of Graph: {}", x,),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "value" => self.value.borrow().to_value(),
            x => panic!("Tried to access inexistant property of Graph: {}", x,),
        }
    }
}

#[object_subclass]
impl ObjectSubclass for GraphPriv {
    type ParentType = gtk::Bin;
    type Type = Graph;

    const NAME: &'static str = "Graph";
}

impl Graph {
    pub fn new() -> Self {
        glib::Object::new::<Self>(&[]).expect("Failed to create MyAwesome Widget")
    }
}

impl ContainerImpl for GraphPriv {
    fn add(&self, container: &Self::Type, widget: &gtk::Widget) {
        self.parent_add(container, widget);
        self.content.replace(Some(widget.clone()));
    }
}

impl BinImpl for GraphPriv {}
impl WidgetImpl for GraphPriv {
    // https://sourcegraph.com/github.com/GNOME/fractal/-/blob/fractal-gtk/src/widgets/clip_container.rs?L119 ???
    // https://stackoverflow.com/questions/50283367/drawingarea-fill-area-outside-a-region
    fn draw(&self, widget: &Self::Type, cr: &cairo::Context) -> Inhibit {
        let styles = widget.style_context();

        let width = widget.allocated_width() as f64;
        let height = widget.allocated_height() as f64;

        let res: Result<()> = try {

            cr.save()?;

            // TODO:

            cr.restore()?;
        };

        if let Err(error) = res {
            error_handling_ctx::print_error(error)
        };

        if let Some(child) = &*self.content.borrow() {
            widget.propagate_draw(child, &cr);
        }
        gtk::Inhibit(false)
    }
}


