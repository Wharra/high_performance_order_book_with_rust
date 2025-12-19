// Price-indexed arrays + bitset for fast scanning

use crate::interfaces::{OrderBook, Price, Quantity, Side, Update};

const MAX_PRICE: usize = 200_001;
const BLOCK_SIZE: usize = 64;
const NUM_BLOCKS: usize = (MAX_PRICE + BLOCK_SIZE - 1) / BLOCK_SIZE;

pub struct OrderBookImpl {
    // Price-indexed arrays: bids[price] = quantity (0 if empty)
    bids: Vec<Quantity>,
    asks: Vec<Quantity>,
    
    // Bitsets: one bit per price level, 64 prices per block
    bitmask_bid: Vec<u64>,
    bitmask_ask: Vec<u64>,
    
    // Cached best prices (-1 if empty)
    best_bid: i64,
    best_ask: i64,
    
    // Cached total quantities
    total_bid_quantity: Quantity,
    total_ask_quantity: Quantity,
}

impl OrderBookImpl {
    #[inline(always)]
    fn get_bid(&self, price: Price) -> Quantity {
        unsafe { *self.bids.get_unchecked(price as usize) }
    }
    
    #[inline(always)]
    fn get_ask(&self, price: Price) -> Quantity {
        unsafe { *self.asks.get_unchecked(price as usize) }
    }
    
    #[inline(always)]
    fn set_bid(&mut self, price: Price, qty: Quantity) {
        unsafe { *self.bids.get_unchecked_mut(price as usize) = qty; }
    }
    
    #[inline(always)]
    fn set_ask(&mut self, price: Price, qty: Quantity) {
        unsafe { *self.asks.get_unchecked_mut(price as usize) = qty; }
    }
    
    #[inline(always)]
    fn update_bitmask_bid(&mut self, price: Price, has_qty: bool) {
        let price_usize = price as usize;
        let block = price_usize / BLOCK_SIZE;
        let bit = price_usize % BLOCK_SIZE;
        let mask = 1u64 << bit;
        
        if has_qty {
            unsafe { *self.bitmask_bid.get_unchecked_mut(block) |= mask; }
        } else {
            unsafe { *self.bitmask_bid.get_unchecked_mut(block) &= !mask; }
        }
    }
    
    #[inline(always)]
    fn update_bitmask_ask(&mut self, price: Price, has_qty: bool) {
        let price_usize = price as usize;
        let block = price_usize / BLOCK_SIZE;
        let bit = price_usize % BLOCK_SIZE;
        let mask = 1u64 << bit;
        
        if has_qty {
            unsafe { *self.bitmask_ask.get_unchecked_mut(block) |= mask; }
        } else {
            unsafe { *self.bitmask_ask.get_unchecked_mut(block) &= !mask; }
        }
    }
    
    #[inline(always)]
    fn recompute_best_bid(&mut self) {
        let start_block = ((self.best_bid.max(0) as usize) / BLOCK_SIZE).min(NUM_BLOCKS - 1);
        let mut block = start_block;
        
        let mask = unsafe { *self.bitmask_bid.get_unchecked(block) };
        if mask != 0 {
            let bit = 63 - mask.leading_zeros() as usize;
            self.best_bid = (block * BLOCK_SIZE + bit) as i64;
            return;
        }
        
        while block > 0 {
            block -= 1;
            let mask = unsafe { *self.bitmask_bid.get_unchecked(block) };
            if mask != 0 {
                let bit = 63 - mask.leading_zeros() as usize;
                self.best_bid = (block * BLOCK_SIZE + bit) as i64;
                return;
            }
        }
        
        self.best_bid = -1;
    }
    
    #[inline(always)]
    fn recompute_best_ask(&mut self) {
        let start_block = ((self.best_ask.max(0) as usize) / BLOCK_SIZE).min(NUM_BLOCKS - 1);
        let mut block = start_block;
        
        let mask = unsafe { *self.bitmask_ask.get_unchecked(block) };
        if mask != 0 {
            let bit = mask.trailing_zeros() as usize;
            self.best_ask = (block * BLOCK_SIZE + bit) as i64;
            return;
        }
        
        while block < NUM_BLOCKS - 1 {
            block += 1;
            let mask = unsafe { *self.bitmask_ask.get_unchecked(block) };
            if mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                self.best_ask = (block * BLOCK_SIZE + bit) as i64;
                return;
            }
        }
        
        self.best_ask = -1;
    }
}

impl OrderBook for OrderBookImpl {
    #[inline]
    fn new() -> Self {
        OrderBookImpl {
            bids: vec![0; MAX_PRICE],
            asks: vec![0; MAX_PRICE],
            bitmask_bid: vec![0; NUM_BLOCKS],
            bitmask_ask: vec![0; NUM_BLOCKS],
            best_bid: -1,
            best_ask: -1,
            total_bid_quantity: 0,
            total_ask_quantity: 0,
        }
    }

    #[inline(always)]
    fn apply_update(&mut self, update: Update) {
        match update {
            Update::Set {
                price,
                quantity,
                side,
            } => {
                if quantity == 0 {
                    match side {
                        Side::Bid => {
                            let old_qty = self.get_bid(price);
                            if old_qty > 0 {
                                self.set_bid(price, 0);
                                self.update_bitmask_bid(price, false);
                                self.total_bid_quantity -= old_qty;
                                
                                if price == self.best_bid {
                                    self.recompute_best_bid();
                                }
                            }
                        }
                        Side::Ask => {
                            let old_qty = self.get_ask(price);
                            if old_qty > 0 {
                                self.set_ask(price, 0);
                                self.update_bitmask_ask(price, false);
                                self.total_ask_quantity -= old_qty;
                                
                                if price == self.best_ask {
                                    self.recompute_best_ask();
                                }
                            }
                        }
                    }
                    return;
                }
                
                match side {
                    Side::Bid => {
                        let old_qty = self.get_bid(price);
                        let was_new = old_qty == 0;
                        
                        self.set_bid(price, quantity);
                        
                        if was_new {
                            self.update_bitmask_bid(price, true);
                        }
                        
                        let diff = quantity as i64 - old_qty as i64;
                        self.total_bid_quantity = (self.total_bid_quantity as i64 + diff) as u64;
                        
                        self.best_bid = self.best_bid.max(price);
                    }
                    Side::Ask => {
                        let old_qty = self.get_ask(price);
                        let was_new = old_qty == 0;
                        
                        self.set_ask(price, quantity);
                        
                        if was_new {
                            self.update_bitmask_ask(price, true);
                        }
                        
                        let diff = quantity as i64 - old_qty as i64;
                        self.total_ask_quantity = (self.total_ask_quantity as i64 + diff) as u64;
                        
                        if self.best_ask < 0 {
                            self.best_ask = price;
                        } else {
                            self.best_ask = self.best_ask.min(price);
                        }
                    }
                }
            }
            Update::Remove { price, side } => {
                match side {
                    Side::Bid => {
                        let old_qty = self.get_bid(price);
                        if old_qty > 0 {
                            self.set_bid(price, 0);
                            self.update_bitmask_bid(price, false);
                            self.total_bid_quantity -= old_qty;
                            
                            if price == self.best_bid {
                                self.recompute_best_bid();
                            }
                        }
                    }
                    Side::Ask => {
                        let old_qty = self.get_ask(price);
                        if old_qty > 0 {
                            self.set_ask(price, 0);
                            self.update_bitmask_ask(price, false);
                            self.total_ask_quantity -= old_qty;
                            
                            if price == self.best_ask {
                                self.recompute_best_ask();
                            }
                        }
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn get_spread(&self) -> Option<Price> {
        let bid = self.best_bid;
        let ask = self.best_ask;
        if bid >= 0 && ask >= 0 {
            Some(ask - bid)
        } else {
            None
        }
    }

    #[inline(always)]
    fn get_best_bid(&self) -> Option<Price> {
        let bid = self.best_bid;
        if bid >= 0 {
            Some(bid)
        } else {
            None
        }
    }

    #[inline(always)]
    fn get_best_ask(&self) -> Option<Price> {
        let ask = self.best_ask;
        if ask >= 0 {
            Some(ask)
        } else {
            None
        }
    }

    #[inline(always)]
    fn get_quantity_at(&self, price: Price, side: Side) -> Option<Quantity> {
        match side {
            Side::Bid => {
                let qty = self.get_bid(price);
                if qty > 0 {
                    Some(qty)
                } else {
                    None
                }
            }
            Side::Ask => {
                let qty = self.get_ask(price);
                if qty > 0 {
                    Some(qty)
                } else {
                    None
                }
            }
        }
    }

    fn get_top_levels(&self, side: Side, n: usize) -> Vec<(Price, Quantity)> {
        match side {
            Side::Bid => {
                let mut result = Vec::with_capacity(n);
                if self.best_bid < 0 {
                    return result;
                }
                
                let mut count = 0;
                let mut p = self.best_bid;
                
                while p >= 0 && count < n {
                    let qty = self.get_bid(p);
                    if qty > 0 {
                        result.push((p, qty));
                        count += 1;
                    }
                    p -= 1;
                }
                
                result
            }
            Side::Ask => {
                let mut result = Vec::with_capacity(n);
                if self.best_ask < 0 {
                    return result;
                }
                
                let mut count = 0;
                let mut p = self.best_ask;
                
                while p < MAX_PRICE as i64 && count < n {
                    let qty = self.get_ask(p);
                    if qty > 0 {
                        result.push((p, qty));
                        count += 1;
                    }
                    p += 1;
                }
                
                result
            }
        }
    }

    #[inline(always)]
    fn get_total_quantity(&self, side: Side) -> Quantity {
        match side {
            Side::Bid => self.total_bid_quantity,
            Side::Ask => self.total_ask_quantity,
        }
    }
}
