#![feature(allocator_api)]

use std::alloc::Global;

use zk_ee::common_structs::history_map::*;

#[test]
fn miri_rollback_reuse() {
    let mut map = HistoryMap::<usize, usize, Global>::new(Global);

    map.snapshot();

    let mut v = map.get_or_insert::<()>(&1, || Ok(1)).unwrap();

    v.update::<_, ()>(|x| {
        *x = 2;
        Ok(())
    })
    .unwrap();

    // We'll rollback to this point.
    let ss = map.snapshot();

    let mut v = map.get_or_insert::<()>(&1, || Ok(4)).unwrap();

    // This snapshot will be rolled back.
    v.update::<_, ()>(|x| {
        *x = 3;
        Ok(())
    })
    .unwrap();

    // Just for fun.
    map.snapshot();

    map.rollback(ss).expect("Correct snapshot");

    let mut v = map.get_or_insert::<()>(&1, || Ok(5)).unwrap();

    // This will create a new snapshot and will reuse the one that rolled back.
    v.update::<_, ()>(|x| {
        *x = 6;
        Ok(())
    })
    .unwrap();

    map.apply_to_all_updated_elements::<_, ()>(|l, r, k| {
        assert_eq!(1, *l);
        assert_eq!(6, *r);
        assert_eq!(1, *k);

        Ok(())
    })
    .unwrap();
}
