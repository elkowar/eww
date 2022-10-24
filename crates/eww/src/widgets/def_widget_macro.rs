#[macro_export]
macro_rules! def_widget {
    ($args:ident, $scope_graph:ident, $gtk_widget:ident, {
        $(
            prop($(
                $attr_name:ident : $typecast_func:ident $(? $(@ $optional:tt @)?)? $(= $default:expr)?
            ),*) $code:block
        ),+ $(,)?
    }) => {
        $({
            $(
                // explicitly box the function to not cause tons of monomorphization related duplications of Vec::retain
                let retain_fn: Box<dyn Fn(&eww_shared_util::wrappers::AttrName, &mut yuck::config::attributes::AttrEntry) -> bool> =
                    Box::new(|a, _| &a.0 != &::std::stringify!($attr_name).replace('_', "-"));
                $args.unhandled_attrs.retain(retain_fn);
            )*

            // Map of all attributes to their provided expressions.
            // If an attribute is explicitly marked as optional (? appended to type)
            // the attribute will still show up here, as a `None` value. Otherwise, all values in this map
            // will be `Some`.
            let attr_map: Result<HashMap<eww_shared_util::AttrName, Option<simplexpr::SimplExpr>>> = try {
                ::maplit::hashmap! {
                    $(
                        eww_shared_util::AttrName(::std::stringify!($attr_name).to_owned()) =>
                            def_widget!(@get_value $args, &::std::stringify!($attr_name).replace('_', "-"), $(? $($optional)?)? $(= $default)?)
                    ),*
                }
            };

            // Only proceed if any attributes from this `prop` where actually provided
            if let Ok(attr_map) = attr_map {
                if attr_map.values().any(|x| x.is_some()) {

                    // Get all the variables that are referred to in any of the attributes expressions
                    let required_vars: Vec<eww_shared_util::VarName> = attr_map
                        .values()
                        .flat_map(|expr| expr.as_ref().map(|x| x.collect_var_refs()).unwrap_or_default())
                        .collect();

                    $args.scope_graph.register_listener(
                        $args.calling_scope,
                            $crate::state::scope::Listener {
                            needed_variables: required_vars,
                            f: Box::new({
                                // create a weak reference to the widget, such that this listener doesn't prevent the actual widget from
                                // getting deallocated (garbage collected by the gtk runtime)
                                let $gtk_widget = gdk::glib::clone::Downgrade::downgrade(&$gtk_widget);
                                move |$scope_graph, values| {
                                    // TODO when this fails, shouldn't we technically remove the listener somehow? Need to analyze when exactly this happens.
                                    let $gtk_widget = gdk::glib::clone::Upgrade::upgrade(&$gtk_widget)
                                        .context("Couldn't upgrade reference, widget got deallocated")?;
                                    // values is a map of all the variables that are required to evaluate the
                                    // attributes expression.


                                    // We first initialize all the local variables for all the expected attributes in scope
                                    $(
                                        // get the simplexprs from the attr_map
                                        let $attr_name = attr_map.get(::std::stringify!($attr_name))
                                            .context("Missing attribute, this should never happen")?;

                                        // if the value is Some, evaluate and typecast it as expected
                                        let $attr_name = if let Some(x) = $attr_name {
                                            Some(x.eval(&values)?.$typecast_func()?)
                                        } else {
                                            None
                                        };
                                        // If the attribute is optional, keep it as Option<T>, otherwise unwrap
                                        // because we _know_ the value in the attr_map is Some if the attribute is not optional.
                                        def_widget!(@unwrap_if_required $attr_name $(? $($optional)?)?);
                                    )*

                                    // And then run the provided code with those attributes in scope.
                                    $code
                                    Ok(())
                                }
                            }),
                        },
                    )?;
                }
            }
        })+
    };

    (@unwrap_if_required $value:ident ?) => { };
    (@unwrap_if_required $value:ident) => {
        let $value = $value.unwrap();
    };

    // The attribute is explicitly marked as optional - the value should be provided to the prop function body as Option<T>
    (@get_value $args:ident, $name:expr, ?) => {
        $args.widget_use.attrs.ast_optional::<simplexpr::SimplExpr>($name)?.clone()
    };

    // The attribute has a default value
    (@get_value $args:ident, $name:expr, = $default:expr) => {
        Some($args.widget_use.attrs.ast_optional::<simplexpr::SimplExpr>($name)?.clone().unwrap_or_else(|| simplexpr::SimplExpr::synth_literal($default)))
    };

    // The attribute is required - the prop will only be ran if this attribute is actually provided.
    (@get_value $args:ident, $name:expr,) => {
        Some($args.widget_use.attrs.ast_required::<simplexpr::SimplExpr>($name)?.clone())
    }
}
