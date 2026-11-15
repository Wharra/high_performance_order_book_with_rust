# High-Performance Order Book in Rust

Course assignment (TD) from a low-latency programming class at ESILV, run as a speed competition.
The teaching team provided the interface, benchmark harness and tests; **my implementation is in
`src/orderbook.rs`**. The goal: make the hot path (`apply_update`, `get_spread`) as fast as possible.

## Project structure

```
src/
├── orderbook.rs     # My implementation (price-indexed arrays + bitsets)
├── interfaces.rs    # OrderBook trait and types (provided)
├── benchmarks.rs    # Benchmark harness (provided)
└── main.rs          # Entry point, benchmarks and correctness tests (provided)
```

## Design

**Price-indexed arrays instead of trees.** Bids and asks are stored in flat `Vec<u64>`
indexed directly by price. Updates and lookups are O(1), with no allocation on the hot path.

**64-bit bitsets for best-price lookup.** Each side keeps a bitmask with one bit per price level
(64 levels per block). When the best level is removed, the next best price is found with a single
`leading_zeros` (bids) or `trailing_zeros` (asks) instruction per block, instead of scanning prices.

**Cached state.** Best bid, best ask and total quantity per side are maintained incrementally,
so `get_spread`, `get_best_bid/ask` and `get_total_quantity` are O(1) reads.

**Hot-path tuning.** `#[inline(always)]` on the update path and unchecked indexing
to remove bounds checks inside the critical loop.

## Benchmarks

Measured with the provided harness, `cargo run --release`, 100,000 operations:

| Operation       | Latency  |
|-----------------|----------|
| Update (avg)    | 0.87 ns  |
| Update (P50)    | 0.75 ns  |
| Update (P95)    | 1.38 ns  |
| Update (P99)    | 1.67 ns  |
| Get best bid    | 0.31 ns  |
| Get best ask    | 0.31 ns  |
| Get spread      | 0.32 ns  |
| Random read     | 0.32 ns  |

Note: the harness repeats each update in batches of 1,000 to amortise timer overhead, so these
figures reflect cache-hot, amortised throughput rather than isolated single-operation latency.

## Trade-offs and next steps

- Memory scales with the price range (fixed `MAX_PRICE`), which suits a bounded tick grid
  but not an unbounded one.
- Unchecked indexing assumes prices are validated upstream. A production version would
  validate input at the boundary.
- `get_top_levels` scans price by price; it could reuse the bitsets to skip empty blocks.
- Next step: measure single-operation latency on randomised updates (cold cache, varying prices)
  to get a more realistic picture.

## Run

```bash
cargo test            # correctness
cargo run --release   # benchmarks
```
