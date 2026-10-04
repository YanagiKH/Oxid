# Transfer event versus destination snapshot identity

The correction's physical destination Uninitialized/g1 is also the destination identity retained by the transfer's write-begin/write-end storage snapshots. The published array_observe.rs StorageSnapshot contract intentionally identifies the pre-write destination. Those snapshots do not change to g2 merely because the eventual owner becomes Available/g2.

The Transfer event deliberately uses the installed destination epoch g2. Thus the same successful move has source Available/g2, a physical destination and transfer storage snapshots at Uninitialized/g1, an Event::Transfer destination g2, then source Moved/g3 and destination Available/g2. This distinction applies to all five checkpoint01 fixture moves and introduces no new raw operation or fuel charge.

This is static source/spec clarification only, bound to published bb2b5d7c39d5940cbbf959444646c2dacb8edcb4. No candidate observation was used. Original checkpoint01 and the two correction JSON files remain unchanged.
