#![feature(allocator_api)]

use std::alloc::Global;

use zk_ee::common_structs::history_map::*;

#[test]
fn miri_rollback_reuse() {
    let mut map = HistoryMap::<usize, usize, (), Global>::new(Global);

    map.snapshot(TransactionId(1));

    let mut v = map
        .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((1, Appearance::Retrieved)))
        .unwrap();

    v.update(|x, _| {
        *x = 2;
        Ok(())
    })
    .unwrap();

    // We'll rollback to this point.
    let ss = map.snapshot(TransactionId(1));

    let mut v = map
        .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((4, Appearance::Retrieved)))
        .unwrap();

    // This snapshot will be rollbacked.
    v.update(|x, _| {
        *x = 3;
        Ok(())
    })
    .unwrap();

    // Just for fun.
    map.snapshot(TransactionId(1));

    map.rollback(ss);

    let mut v = map
        .get_or_insert::<_, ()>(&mut 0, &1, |_| Ok((5, Appearance::Retrieved)))
        .unwrap();

    // This will create a new snapshot and will reuse the one that rollbacked.
    v.update(|x, _| {
        *x = 6;
        Ok(())
    })
    .unwrap();

    map.for_total_diff_operands::<_, ()>(|l, r, k| {
        assert_eq!(1, l.value);
        assert_eq!(6, r.value);
        assert_eq!(1, *k);

        Ok(())
    })
    .unwrap();
}
