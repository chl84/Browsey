//! Exercise the upstream GLib soundness fix with the application's actual ABI.
use gio::glib::variant::ToVariant;

#[test]
fn glib_variant_string_iterator_out_pointer_is_mutable() {
    let variant = vec!["first", "æ #?%", "last"].to_variant();
    let mut iterator = variant.array_iter_str().unwrap();
    assert_eq!(iterator.next(), Some("first"));
    assert_eq!(iterator.next_back(), Some("last"));
    assert_eq!(iterator.next(), Some("æ #?%"));
    assert_eq!(iterator.next(), None);
    assert_eq!(variant.array_iter_str().unwrap().nth(1), Some("æ #?%"));
    assert_eq!(variant.array_iter_str().unwrap().last(), Some("last"));
    assert_eq!(
        variant.array_iter_str().unwrap().collect::<Vec<_>>(),
        ["first", "æ #?%", "last"]
    );
}
