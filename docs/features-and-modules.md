# Features and Modules
#define dbg_assert(expr) dbg_discard_expr_("%d", !(expr))
#define dbg_ensures(expr) dbg_discard_expr_("%d", !(expr))
#define dbg_printf(...) dbg_discard_expr_(__VA_ARGS__)
#define dbg_printheap(...) ((void)((0) && print_heap(__VA_ARGS__)))
#endif

/* Basic constants */

typedef uint64_t word_t;

/** @brief Word and header size (bytes) */
static const size_t wsize = sizeof(word_t);

/** @brief Double word size (bytes) */
static const size_t dsize = 2 * wsize;

/** @brief Minimum block size (bytes) */
static const size_t min_block_size = dsize;

/** @brief Minimum free block size (bytes) - blocks this size and larger get footers and prev/next pointers */
static const size_t min_free_block = 4 * wsize;

/**
 * @brief Extend heap by this amount (bytes)
 */
static const size_t chunksize = (1 << 12);

/**
 * @brief Mask to set/get alloc/free bit
 */
static const word_t alloc_mask = 0x1;

/**
 * @brief Mask to set/get the previous block's alloc/free bit
 */
static const word_t prev_alloc_mask = 0x2;

/**
 * @brief Mask to set/get if previous block is a mini block
 */
static const word_t prev_mini_mask = 0x4;

/**
 * @brief Mask to set/get size of block (bytes)
 */
static const word_t size_mask = ~(word_t)0xF;


/** @brief Represents the header and payload of one block in the heap */
typedef struct block {
    /** @brief Header contains size + allocation flag */
    word_t header;

    /*
     * @brief Union to handle different payload interpretations
     */
    union {
        char raw[0];                    // For allocated blocks - raw user data
        struct {                        // For free blocks >= min_free_block
            struct block *next;
            struct block *prev;
        } free_ptrs;
        struct {                        // For mini blocks
            struct block *next;
        } mini_ptr;
    } payload;
} block_t;


/* Global variables */

/** @brief Pointer to first block in the heap */
static block_t *heap_start = NULL;

/** @brief Static array of DLLs for differently sized free blocks */
static block_t *seg_lists[NUM_BUCKETS];

/*
 *****************************************************************************
 * The functions below are short wrapper functions to perform                *
 * bit manipulation, pointer arithmetic, and other helper operations.        *
 *                                                                           *
 * We've given you the function header comments for the functions below      *
 * to help you understand how this baseline code works.                      *
 *                                                                           *
 * Note that these function header comments are short since the functions    *
 * they are describing are short as well; you will need to provide           *
 * adequate details for the functions that you write yourself!               *
 *****************************************************************************
 */

/*
 * ---------------------------------------------------------------------------
 *                        BEGIN SHORT HELPER FUNCTIONS
 * ---------------------------------------------------------------------------
 */

/**
 * @brief Returns the maximum of two integers.
 * @param[in] x
 * @param[in] y
 * @return `x` if `x > y`, and `y` otherwise.
 */
static size_t max(size_t x, size_t y) {
    return (x > y) ? x : y;
}

/**
 * @brief Rounds `size` up to next multiple of n
 * @param[in] size
 * @param[in] n
 * @return The size after rounding up
 */
static size_t round_up(size_t size, size_t n) {
    return n * ((size + (n - 1)) / n);
}

/**
 * @brief Packs the `size` and `alloc` of a block into a word suitable for
 *        use as a packed value.
 *
 * Packed values are used for both headers and footers.
 *
 * The allocation status is packed into the lowest bit of the word.
 * The previous blocks' allocation status is packed into the second lowest bit
 * The previous block mini status is packed into the third lowest bit
 *
 * @param[in] size The size of the block being represented
 * @param[in] alloc True if the block is allocated
 * @param[in] prev_alloc True if the previous block is allocated
 * @param[in] prev_mini True if the previous block is a mini block
 * @return The packed value
 */
static word_t pack(size_t size, bool alloc, bool prev_alloc, bool prev_mini) {
    word_t word = size;
    if (alloc)
        word |= alloc_mask;
    if (prev_alloc)
        word |= prev_alloc_mask;
    if (prev_mini)
        word |= prev_mini_mask;
    return word;
}

/**
 * @brief extracts the previous allocation status of a block from its header.
 */
static bool get_prev_alloc(block_t *block) {
    return (bool)(block->header & prev_alloc_mask);
}

/**
 * @brief extracts the previous mini status of a block from its header.
 */
static bool get_prev_mini(block_t *block) {
    return (bool)(block->header & prev_mini_mask);
}

/**
 * @brief Extracts the size represented in a packed word.
 *
 * This function simply clears the lowest 4 bits of the word, as the heap
 * is 16-byte aligned.
 *
 * @param[in] word
 * @return The size of the block represented by the word
 */
static size_t extract_size(word_t word) {
    return (word & size_mask);
}

/**
 * @brief Extracts the size of a block from its header.
 * @param[in] block
 * @return The size of the block
 */
static size_t get_size(block_t *block) {
    return extract_size(block->header);
}

/**
 * @brief Given a payload pointer, returns a pointer to the corresponding
 *        block.
 * @param[in] bp A pointer to a block's payload
 * @return The corresponding block
 */
static block_t *payload_to_header(void *bp) {
    return (block_t *)((char *)bp - offsetof(block_t, payload.raw));
}

/**
 * @brief Given a block pointer, returns a pointer to the corresponding
 *        payload.
 * @param[in] block
 * @return A pointer to the block's payload
 * @pre The block must be a valid block, not a boundary tag.
 */
static void *header_to_payload(block_t *block) {
    dbg_requires(get_size(block) != 0);
    return (void *)(block->payload.raw);
}

/**
 * @brief Given a block pointer, returns a pointer to the corresponding
 *        footer.
 * @param[in] block
 * @return A pointer to the block's footer
 * @pre The block must be a valid block, not a boundary tag.
 */
static word_t *header_to_footer(block_t *block) {
    dbg_requires(get_size(block) != 0 &&
                 "Called header_to_footer on the epilogue block");
    return (word_t *)(block->payload.raw + get_size(block) - dsize);
}

/**
 * @brief Given a block footer, returns a pointer to the corresponding
 *        header.
 * @param[in] footer A pointer to the block's footer
 * @return A pointer to the start of the block
 * @pre The footer must be the footer of a valid block, not a boundary tag.
 */
static block_t *footer_to_header(word_t *footer) {
    size_t size = extract_size(*footer);
    dbg_assert(size != 0 && "Called footer_to_header on the prologue block");
    return (block_t *)((char *)footer + wsize - size);
}


/**
 * @brief Returns the allocation status of a given header value.
 *
 * This is based on the lowest bit of the header value.
 *
 * @param[in] word
 * @return The allocation status correpsonding to the word
 */
static bool extract_alloc(word_t word) {
    return (bool)(word & alloc_mask);
}

/**
 * @brief Returns the allocation status of a block, based on its header.
 * @param[in] block
 * @return The allocation status of the block
 */
static bool get_alloc(block_t *block) {
    return extract_alloc(block->header);
}

/**
 * @brief Returns the payload size of a given block.
 *
 * If the block is free:
 * The payload size is equal to the entire block size minus the sizes of the
 * block's header and footer
 *
 * Otherwise (allocated block):
 * the payload size is equal to the entire block size minus the
 * size of the block's header
 *
 * @param[in] block
 * @return The size of the block's payload
 */
static size_t get_payload_size(block_t *block) {
    size_t asize = get_size(block);
    if (get_alloc(block)) {
        return asize - wsize; // allocated blocks only have a header
    } else {
        return asize - dsize; // free blocks have header and footer
    }
}

/**
 * @brief Writes an epilogue header at the given address.
 *
 * The epilogue header has size 0, and is marked as allocated.
 *
 * @param[out] block The location to write the epilogue header
 * @param[in] prev_is_alloc True if the previous block is allocated
 * @param[in] prev_is_mini True if the previous block is a mini block
 */
static void write_epilogue(block_t *block, bool prev_is_alloc, bool prev_is_mini) {
    dbg_requires(block != NULL);
    dbg_requires((char *)block == (char *)mem_heap_hi() + 1 - wsize); // Should be at the very end

    block->header = pack(0, true, prev_is_alloc, prev_is_mini);
}
/**
 * @brief Writes a block starting at the given address.
 *
 * This function writes both a header and footer (if the block is free), where the location of the
 * footer is computed in relation to the header.
 *
 *
 *
 * @param[out] block The location to begin writing the block header
 * @param[in] size The size of the new block
 * @param[in] alloc The allocation status of the new block
 * @param[in] prev_alloc The allocation status of the previous block
 * @param[in] prev_mini True if the previous block is a mini block
 */
static void write_block(block_t *block, size_t size, bool alloc, bool prev_alloc, bool prev_mini) {
    dbg_requires(block != NULL);
    dbg_requires(size > 0);
    block->header = pack(size, alloc, prev_alloc, prev_mini);
    if (!alloc && size >= min_free_block) {
        word_t *footerp = header_to_footer(block);
        *footerp = pack(size, alloc, prev_alloc, prev_mini);
    }
}

/**
 * @brief Finds the next consecutive block on the heap.
 *
 * This function accesses the next block in the "implicit list" of the heap
 * by adding the size of the block.
 *
 * @param[in] block A block in the heap
 * @return The next consecutive block on the heap
 * @pre The block is not the epilogue
 */
static block_t *find_next(block_t *block) {
    dbg_requires(block != NULL);
    dbg_requires(get_size(block) != 0 &&
                 "Called find_next on the last block in the heap");
    return (block_t *)((char *)block + get_size(block));
}

/**
 * @brief Finds the footer of the previous block on the heap.
 * @param[in] block A block in the heap
 * @return The location of the previous block's footer
 */
static word_t *find_prev_footer(block_t *block) {
    // Compute previous footer position as one word before the header
    return &(block->header) - 1;
}

/**
 * @brief Finds the previous consecutive block on the heap.
 *
 * This is the previous block in the "implicit list" of the heap.
 *
 * If the function is called on the first block in the heap, NULL will be
 * returned, since the first block in the heap has no previous block!
 *
 * The position of the previous block is found by reading the previous
 * block's footer to determine its size, then calculating the start of the
 * previous block based on its size.
 *
 * @param[in] block A block in the heap
 * @return The previous consecutive block in the heap.
 */
static block_t *find_prev(block_t *block)
{
    if (block == heap_start) {
        return NULL;                     // nothing before the first block
    }

    if (get_prev_mini(block)) {
        // Previous block is a mini block - just step back 16 bytes
        return (block_t *)((char *)block - min_block_size);
    }

    if (get_prev_alloc(block)) {
        return NULL;                     // previous block is allocated
    }

    // Use footer method for normal free blocks
    word_t *footerp = find_prev_footer(block);
    size_t size = extract_size(*footerp);

    /* size==0 ⇒ prologue footer, not a real block */
    if (size == 0) {
        return NULL;
    }

    return footer_to_header(footerp);
}





/**
 * @brief Gets bucket based on log_2(size) - optimized for throughput
 * @param[in] size Size of free block
 * @return Index in seg_lists that fits size
 */
static size_t get_bucket(size_t size) {
    /* Bucket 0 stores mini blocks.  Use min_block_size instead of a magic
       constant so the allocator remains correct if alignment or word size
       changes. */
    if (size == min_block_size) return 0;  // Mini blocks get bucket 0
    size >>= 5;  // Start log₂ from 32 B
    size_t bucket = 1;
    while (size > 0 && bucket < NUM_BUCKETS - 1) {
        size >>= 1;
        bucket++;
    }
    return bucket;
}


/**
 * @brief Gets bucket size
 * Roughly powers of 2
 * @param[in] size Size of free block
 * @return Index in seg_lists that fits size
 */
/*
static size_t get_bucket(size_t size) {
    if (size <= 16)    return 0;   // Mini blocks
    if (size <= 24)    return 1;
    if (size <= 32)    return 2;
    if (size <= 48)    return 3;
    if (size <= 64)    return 4;
    if (size <= 96)    return 5;
    if (size <= 128)   return 6;
    if (size <= 192)   return 7;
    if (size <= 256)   return 8;
    if (size <= 384)   return 9;
    if (size <= 512)   return 10;
    if (size <= 1024)  return 11;
    if (size <= 2048)  return 12;
    if (size <= 4096)  return 13;
    if (size <= 8192)  return 14;
    return 15;
}
*/

/**
 * @brief Checks if block is epilogue block
 * @param[in] block a Block in the heap
 * @return whether or not the block is the epilogue
 */
static bool is_epilogue(block_t *block) {
    return get_size(block) == 0;
}

/**
 * @brief Extracts prev ptr from free block
 * @param[in] block a Free Block in the heap
 * @return block pointer to prev free block or NULL if no prev free block
 */
static block_t *get_prev_free(block_t *block) {
    dbg_requires(!get_alloc(block));
    dbg_requires(get_size(block) >= min_free_block);
    return block->payload.free_ptrs.prev;
}

/**
 * @brief Extracts next ptr from free block
 * @param[in] block a Free Block in the heap
 * @return block pointer to next free block or NULL if no next free block
 */
static block_t *get_next_free(block_t *block) {
    dbg_requires(!get_alloc(block));
    dbg_requires(!is_epilogue(block));
    if (get_size(block) >= min_free_block) {
        return block->payload.free_ptrs.next;
    } else {
        return block->payload.mini_ptr.next;
    }
}

/**
 * @brief Sets prev ptr from free block
 * @param[in] block a Free Block in the heap
 * @param[in] block the prev Free block in the explicit free list
 * @return None
 */
static void set_prev_free(block_t *block, block_t *prev) {
    dbg_requires(!get_alloc(block));
    dbg_requires(get_size(block) >= min_free_block);
    block->payload.free_ptrs.prev = prev;
}

/**
 * @brief Extracts next ptr from free block
 * @param[in] block a Free Block in the heap
 * @param[in] block the next Free block in the explicit free list
 * @return None
 */
static void set_next_free(block_t *block, block_t *next) {
    dbg_requires(!get_alloc(block));
    dbg_requires(!is_epilogue(block));
    if (get_size(block) >= min_free_block) {
        block->payload.free_ptrs.next = next;
    } else {
        block->payload.mini_ptr.next = next;
    }
}

/**
 * @brief Inserts a free block into the free block DLL
 * @param[in] block a Free Block in the heap
 * @return None
 */
static void insert_free(block_t *block) {
    dbg_requires(!get_alloc(block));

    // retrieve correct segregated list
    size_t size = get_size(block);
    size_t bucket = get_bucket(size);
    block_t *head = seg_lists[bucket];

    // handle DLL linking
    set_next_free(block, head);
    if (size >= min_free_block) {
        set_prev_free(block, NULL);
    }

    if (head != NULL && get_size(head) >= min_free_block) { // don't set for miniblocks
        set_prev_free(head, block); // link prev head
    }

    seg_lists[bucket] = block; // update head
}

/**
 * @brief Removes a given free block from segregated lists
 *
 * Handles removal of regular blocks from the DLLs and miniblocks from SLL
 *
 * @param[in] block a Free Block in the heap
 * @return None
 */
static void remove_free(block_t *block) {
    dbg_requires(!get_alloc(block));

    size_t size   = get_size(block);
    size_t bucket = get_bucket(size);

    /* Fast path: if the block is the bucket head, update the head pointer
       directly without touching the block's next‐pointer first. */
    if (seg_lists[bucket] == block) {
        block_t *new_head = get_next_free(block);
        seg_lists[bucket] = new_head;
        if (size >= min_free_block && new_head != NULL) {
            set_prev_free(new_head, NULL);
        }
    } else if (size < min_free_block) {
        /* Mini blocks form a singly-linked list – walk until we find the
           predecessor. */
        block_t *curr = seg_lists[bucket];
        while (curr != NULL && get_next_free(curr) != block) {
            curr = get_next_free(curr);
        }
        if (curr != NULL) {
            set_next_free(curr, get_next_free(block));
        }
    } else {
        /* Regular free blocks use a doubly-linked list. */
        block_t *prev = get_prev_free(block);
        block_t *next = get_next_free(block);

        if (prev != NULL) {
            set_next_free(prev, next);
        }
        if (next != NULL && get_size(next) >= min_free_block) {
            set_prev_free(next, prev);
        }
        if (seg_lists[bucket] == block) {
            seg_lists[bucket] = next;
        }
    }

    /* Poison the removed block’s pointers so accidental re-use is obvious. */
    set_next_free(block, NULL);
    if (size >= min_free_block) {
        set_prev_free(block, NULL);
    }
}


/*
 * ---------------------------------------------------------------------------
 *                        END SHORT HELPER FUNCTIONS
 * ---------------------------------------------------------------------------
 */

/******** The remaining content below are helper and debug routines ********/

/**
 * @brief Coalesces blocks after free
 * Merges together next and/or prev free block with block
 * @param[in] block
 * @return Coalesced block
 */
static block_t *coalesce_block(block_t *block)
{
    size_t size          = get_size(block);
    bool   prev_alloc    = get_prev_alloc(block);
    block_t *next_block  = find_next(block);
    bool     next_alloc  = get_alloc(next_block);

    /* -------- find previous -------- */
    block_t *prev_block = find_prev(block);

    bool coalesce_prev = (prev_block && !get_alloc(prev_block));
    bool coalesce_next = (!next_alloc && !is_epilogue(next_block));

    /* ---------- merge size ---------- */
    block_t *result_block = block;

    if (coalesce_prev) {
        size += get_size(prev_block);
        result_block = prev_block;
        remove_free(prev_block);
    }
    if (coalesce_next) {
        size += get_size(next_block);
        remove_free(next_block);
    }

    /* ---------- rewrite header/footer ---------- */
    bool result_prev_alloc = coalesce_prev ? get_prev_alloc(prev_block)
                                           : prev_alloc;
    bool result_prev_mini  = coalesce_prev ? get_prev_mini(prev_block)
                                           : get_prev_mini(block);

    write_block(result_block, size, false,
                result_prev_alloc, result_prev_mini);

    /* --------- fix successor metadata ---------- */
    block_t *new_next = find_next(result_block);
    if (!is_epilogue(new_next)) {
        new_next->header &= ~prev_alloc_mask;
        new_next->header &= ~prev_mini_mask;
        /* Correctly set prev_mini bit if the coalesced block is a mini block */
        if (get_size(result_block) == min_block_size) {
            new_next->header |= prev_mini_mask;
        }
        if (!get_alloc(new_next) && get_size(new_next) >= min_free_block) {
            *header_to_footer(new_next) = new_next->header;
        }
    }
    return result_block;
}


/**
 * @brief Increases size of heap
 *
 * Requests additional virtual memory from kernel
 *
 *
 *
 *
 * @param[in] size
 * @return First block in the newly extended heap
 */
static block_t *extend_heap(size_t size) {
    dbg_requires(size > 0);
    void *bp;

    // Read epilogue info before extending heap
    block_t *old_epilogue = (block_t *)((char *)mem_heap_hi() + 1 - wsize);
    bool prev_was_alloc = get_prev_alloc(old_epilogue);
    bool prev_was_mini = get_prev_mini(old_epilogue);

    // Allocate an even number of words to maintain alignment
    size = round_up(size, dsize);
    if ((bp = mem_sbrk((intptr_t)size)) == (void *)-1) {
        return NULL;
    }

    /* --------------------------------------------------------------------
     * 1. Create the new free block just appended to the heap              
     * 2. Immediately write a provisional epilogue header *after* it so    
     *    that helper routines such as find_next() will always encounter   
     *    a valid block header inside the heap bounds.                     
     * 3. Coalesce the new block with its predecessor if that block is     
     *    free.                                                            
     * 4. Re-write the epilogue header, because the coalesced result may   
     *    have a different "mini" status from the provisional value.      
     * ------------------------------------------------------------------*/

    /* Step 1: The start of the new free block is exactly where the old
       epilogue header resided.  That header now becomes the header of the
       free block.  Using the saved pointer avoids subtracting a word from
       the sbrk result, which could underflow past the start of the heap on
       some memory-library implementations (and was the root cause of the
       earlier SEGV). */
    block_t *block = old_epilogue;
    write_block(block, size, false, prev_was_alloc, prev_was_mini);

    /* Step 2: write provisional epilogue header */
    block_t *provisional_epilogue = find_next(block);
    bool block_is_mini = (size == min_block_size);
    write_epilogue(provisional_epilogue, false, block_is_mini);

    /* Step 3: attempt to coalesce with the previous block */
    block_t *coalesced_block = coalesce_block(block);

    /* Step 4: write (or rewrite) the real epilogue header that follows
       the coalesced block so that its metadata accurately reflects the
       final predecessor. */
    block_t *new_epilogue = find_next(coalesced_block);
    bool coalesced_is_mini = (get_size(coalesced_block) == min_block_size);
    write_epilogue(new_epilogue, false, coalesced_is_mini);

    /* Insert the (possibly coalesced) free block into the appropriate
       segregated list. */
    insert_free(coalesced_block);

    return coalesced_block;
}


static void split_block(block_t *block, size_t asize) {
    size_t block_size = get_size(block);
    bool prev_alloc = get_prev_alloc(block);
    bool prev_mini = get_prev_mini(block);

    /* Only split if the remainder is at least min_block_size.  Note that a
       remainder of exactly min_block_size produces a *mini* free block (16
       bytes) which has no footer and only a single next-pointer word in its
       payload. */
    if ((block_size - asize) >= min_block_size) {
        size_t remainder_size = block_size - asize;

        // Write allocated block
        bool alloc_is_mini = (asize == min_block_size);
        write_block(block, asize, true, prev_alloc, prev_mini);

        // Write remainder block
        block_t *block_next = find_next(block);
        write_block(block_next, remainder_size, false, true, alloc_is_mini);

        // Insert the new free block
        insert_free(block_next);

        // Update the metadata for the block after the new free block
        block_t *after_next = find_next(block_next);

        /* Always update the successor’s prev_alloc/prev_mini bits – this
         * includes the epilogue header (size == 0).  Failing to update the
         * epilogue when the remainder is a *mini* block leaves its
         * prev_mini bit stale, which later confuses find_prev() and can
         * corrupt heap traversals.
         */
        after_next->header &= ~prev_alloc_mask;
        after_next->header &= ~prev_mini_mask;
        if (remainder_size == min_block_size) {
            after_next->header |= prev_mini_mask;
        }

        /* If the successor is a free regular block (i.e. has a footer),
         * rewrite its footer to keep header/footer in sync.  The epilogue
         * and mini blocks do not have footers, so we skip them. */
        if (!is_epilogue(after_next) && !get_alloc(after_next) &&
            get_size(after_next) >= min_free_block) {
            *header_to_footer(after_next) = after_next->header;
        }

    } else {
        // Don't split — allocate entire block
        bool alloc_is_mini = (block_size == min_block_size);
        write_block(block, block_size, true, prev_alloc, prev_mini);

        block_t *next_block = find_next(block);
        next_block->header |= prev_alloc_mask;
        if (alloc_is_mini) {
            next_block->header |= prev_mini_mask;
        } else {
            next_block->header &= ~prev_mini_mask;
        }

        if (!get_alloc(next_block) && get_size(next_block) >= min_free_block) {
            *header_to_footer(next_block) = next_block->header;
        }
    }
}

/**
 * @brief
 *
 * Finds free block that can fit asize
 * Uses first fit within buckets for speed
 *
 * @param[in] asize
 * @return block with size that will fit asize
 */
static block_t *find_fit(size_t asize) {
    size_t bucket = get_bucket(asize);
    block_t *best = NULL;
    size_t best_size = SIZE_MAX;

    // Search current bucket with best-fit
    for (block_t *block = seg_lists[bucket]; block != NULL;
         block = get_next_free(block)) {
        size_t bsize = get_size(block);
        if (bsize >= asize && bsize < best_size) {
            best = block;
            best_size = bsize;
            // Early exit if perfect fit
            if (bsize == asize) return best;
        }
    }

    if (best) return best;

    // Fall back to first-fit in larger buckets
    for (bucket++; bucket < NUM_BUCKETS; bucket++) {
        if (seg_lists[bucket]) {
            return seg_lists[bucket];
        }
    }
    return NULL;
}

/**
 * @brief Function to check the validity of a block's header and footer (if present)
 *
 * Checks:
 * the size of the header and footer
 * the alloc bits
 * header matches footer
 *
 *
 * @param[in] block a block on the heap
 * @return bool whether or not header and footer are valid
*/
static bool check_non_payload(block_t *block) {
    // check block's header and footer size (only for free blocks > min_block_size)
    size_t size = get_size(block);
    bool alloc = get_alloc(block);
    bool prev_alloc = get_prev_alloc(block);
    bool prev_mini = get_prev_mini(block);
    word_t expected = pack(size, alloc, prev_alloc, prev_mini);

    if (!is_epilogue(block) && !alloc && get_size(block) >= min_free_block) {
        word_t *footerp = header_to_footer(block);
        if (*footerp != expected) {
            dbg_printf("block header or footer size incorrect \n");
            fflush(stdout);
            return false;
        }

        // check alloc bits
        if ((block->header & alloc_mask) != (*footerp & alloc_mask)) {
            dbg_printf("header/footer alloc mismatch \n");
            fflush(stdout);
            return false;
        }
    }
    return true; // TODO: VERIFY
}


/** @brief Helper function to check alignment of block's payload
 * @param[in] block A block on the heap
 * @return bool whether or not block is properly aligned
 * */
static bool is_aligned(block_t *block) {
    return ((uintptr_t)block->payload.raw % ALIGNMENT) == 0;
}


/**
 * @brief Helper function to verify a block on the heap is correct
 * @param[in] block
 *
 *  Checks:
 *  that block lies within heap range
 *  alignment of block
 *  blocks headers and footer (if present)
 *
 * @return bool whether or not the block is valid
 */
static bool check_block(block_t *block) {
    // check that block lies within heap
    if ((void *)block < mem_heap_lo() || (void *)block > mem_heap_hi()) {
        dbg_printf("block is out of heap bounds\n");
        fflush(stdout);
        return false;
    }

    // check alignment
    if (!is_aligned(block))  {
        dbg_printf("alignment incorrect \n");
        dbg_printf("block address: %p, size: %zu\n", (void *)block, get_size(block));
        dbg_printf("alignment: %lu\n", (uintptr_t)block % ALIGNMENT);
        fflush(stdout);
        return false;
    }

    // check header and footer (when applicable)
    return check_non_payload(block);
}

/**
 * @Brief ensures integrity of the segregated lists
 */
static bool check_segregated_lists() {
    for (size_t i = 0; i < NUM_BUCKETS; i++) {
        block_t *curr = seg_lists[i];
        while (curr != NULL) {
            block_t *next = get_next_free(curr);

            // check bounds of each free list pointer
            if ((void *)curr < mem_heap_lo() || (void *)curr > mem_heap_hi()) {
                dbg_printf("free list pointer out of bounds\n");
                fflush(stdout);
                return false;
            }

            // checks regular block's linkage backwards
            // note: mini blocks are singly linked and don't have a prev pointer.
            if (next != NULL && get_size(next) >= min_free_block) {
                if (get_prev_free(next) != curr) {
                    dbg_printf("free list linkage broken: prev pointer mismatch \n");
                    fflush(stdout);
                    return false;
                }
            }

            if (get_alloc(curr)) { // check blocks are actually free
                dbg_printf("allocated block in free list \n");
                fflush(stdout);
                return false;
            }
            if (get_bucket(get_size(curr)) != i) { // check block's bucket
                dbg_printf("block in wrong bucket \n");
                fflush(stdout);
                return false;
            }
            curr = next;
        }
    }
    return true;
}

/**
 * @brief Function to check the validity of the current heap and segregated lists.
 *
 *  Checks:
 *  heap_start is not null
 *  every block on the heap is valid (payload, alignment, header, footer)
 *  integrity of the segregated lists
 *  integrity of the singly linked miniblock list
 *  epilogue and prologue //TODO: PROLOGUE
 *
 * @param[in] line the line number that calls this function
 * @return bool whether or not any issues are found
 */
bool mm_checkheap(int line) {
    dbg_printf("[Called at line %d]\n", line);

    if (heap_start == NULL) {
        dbg_printf("heap start is NULL \n");
        fflush(stdout);
        return false;
    }

    block_t *block;

    for (block = heap_start; get_size(block) != 0; block = find_next(block)) {
        if (!check_block(block))
            return false;

        // check for consecutive free blocks
        if (!get_alloc(block)) {
            block_t *next = find_next(block);
            if (get_size(next) != 0 && !get_alloc(next)) {
                dbg_printf("consecutive free blocks that should be coalesced \n");
                fflush(stdout);
                return false;
            }
        }
    }

    // check epilogue
    block_t *expected_epilogue = (block_t *)((char *)mem_heap_hi() + 1 - wsize);

    if (!is_epilogue(block)) {
        dbg_printf("Last block is not epilogue\n");
        fflush(stdout);
        return false;
    }
    if (block != expected_epilogue) {
        dbg_printf("Epilogue not at end of heap\n");
        fflush(stdout);
        return false;
    }

    if (block != heap_start) {
        block_t *second_last = find_prev(block);
        if (second_last && find_next(second_last) != block) {
            dbg_printf("Second-to-last block does not lead to epilogue\n");
            return false;
        }
    }

    /* Ensure the epilogue's prev_alloc bit matches the actual allocation
       status of the final real block. */
    block_t *last_real = find_prev(expected_epilogue);

    /* If the epilogue says the previous block is allocated, find_prev() will
       correctly return NULL (because it cannot safely determine the start of
       an allocated block).  In that situation we treat the last block as
       allocated for the purposes of this consistency check. */
    bool last_alloc_status = (last_real == NULL) ? true : get_alloc(last_real);

    if (get_prev_alloc(expected_epilogue) != last_alloc_status) {
        dbg_printf("Epilogue prev_alloc bit stale\n");
        fflush(stdout);
        return false;
    }

    // check DLL linking
    if (!check_segregated_lists()) {
        fflush(stdout);
        return false;
    }


    return true;
}

/**
 * @brief Initializes heap and relevant starting values
 *
 *
 * @return
 */
bool mm_init(void) {
    // Create the initial empty heap
    word_t *start = (word_t *)(mem_sbrk(2 * wsize));

    if (start == (void *)-1) {
        return false;
    }

    // init seg lists
    for (size_t i = 0; i < NUM_BUCKETS; i++) {
        seg_lists[i] = NULL;
    }

    start[0] = pack(0, true, true, false); // Heap prologue (block footer)
    start[1] = pack(0, true, true, false); // Heap epilogue (block header)

    // Heap starts with first "block header", currently the epilogue
    heap_start = (block_t *)&(start[1]);
    // Extend the empty heap with a free block of chunksize bytes
    if (extend_heap(chunksize) == NULL) {
        return false;
    }

    return true;
}

/**
 * @brief Allocates size bytes starting at return addr
 *
 *
 * @param[in] size
 * @return bp Pointer to start of allocated memory
 */
void *malloc(size_t size) {
    dbg_requires(mm_checkheap(__LINE__));

    size_t asize;      // Adjusted block size
    size_t extendsize; // Amount to extend heap if no fit is found
    block_t *block;
    void *bp = NULL;

    // Initialize heap if it isn't initialized
    if (heap_start == NULL) {
        if (!(mm_init())) {
            dbg_printf("Problem initializing heap. Likely due to sbrk");
            return NULL;
        }
    }

    // Ignore spurious requests
    if (size == 0) {
        dbg_ensures(mm_checkheap(__LINE__));
        return NULL;
    }

    // Adjust block size to include overhead and meet alignment requirements.
    asize = round_up(size + wsize, ALIGNMENT); // Add wsize for the header
    if (asize < min_block_size) {
        asize = min_block_size;
    }

    // Search the free list for a fit
    if ((block = find_fit(asize)) != NULL) {
        // A fit was found. Remove it from the free list.
        remove_free(block);
        // Place the block, splitting if necessary.
        split_block(block, asize);
        bp = header_to_payload(block);
    } else {
        // No fit found. Extend the heap.
        extendsize = max(asize, chunksize);
        if ((block = extend_heap(extendsize)) == NULL) {
            return NULL;
        }
        // The block returned by extend_heap is free and already in the free lists.
        remove_free(block);
        // Place the block in the newly extended memory.
        split_block(block, asize);
        bp = header_to_payload(block);
    }

    dbg_ensures(mm_checkheap(__LINE__));
    return bp;
}

/**
 * @brief Frees previously allocated virtual memory
 *
 *
 * @param[in] bp
 */
void free(void *bp) {
    dbg_requires(mm_checkheap(__LINE__));

    if (bp == NULL) {
        return;
    }

    block_t *block = payload_to_header(bp);
    size_t size = get_size(block);

    // The block should be marked as allocated
    dbg_assert(get_alloc(block));

    bool prev_was_alloc = get_prev_alloc(block);
    bool prev_was_mini = get_prev_mini(block);
    bool block_is_mini = (size == min_block_size);

    // mark block as free
    write_block(block, size, false, prev_was_alloc, prev_was_mini);

    // update next block's prev_alloc and prev_mini bits
    block_t *next_block = find_next(block);
    next_block->header &= ~prev_alloc_mask;

    if (block_is_mini) {
        next_block->header |= prev_mini_mask;
    } else {
        next_block->header &= ~prev_mini_mask;
    }

    // if next has footer, update footer to match
    if (!get_alloc(next_block) && get_size(next_block) >= min_free_block) {
        *header_to_footer(next_block) = next_block->header;
    }

    // Coalesce and insert for all blocks
    block_t *coalesced_block = coalesce_block(block);
    insert_free(coalesced_block);

    dbg_ensures(mm_checkheap(__LINE__));
}


/**
 * @brief Increases size of previously allocated virtual memory
 *
 * @param[in] ptr
 * @param[in] size
 * @return pointer to new block of memory that can store size bytes
 */
void *realloc(void *ptr, size_t size) {
    dbg_requires(size > 0);

    block_t *block = payload_to_header(ptr);
    size_t copysize;
    void *newptr;

    // If size == 0, then free block and return NULL
    if (size == 0) {
        free(ptr);
        return NULL;
    }

    // If ptr is NULL, then equivalent to malloc
    if (ptr == NULL) {
        return malloc(size);
    }

    // Otherwise, proceed with reallocation
    newptr = malloc(size);

    // If malloc fails, the original block is left untouched
    if (newptr == NULL) {
        return NULL;
    }

    // Copy the old data
    copysize = get_payload_size(block); // gets size of old payload
    if (size < copysize) {
        copysize = size;
    }
    memcpy(newptr, ptr, copysize);

    // Free the old block
    free(ptr);

    return newptr;
}

/**
 * @brief Allocates and initializes size bytes
 *
 *
 * @param[in] elements
 * @param[in] size
 * @return Pointer to start of allocated virtual memory
 */
void *calloc(size_t elements, size_t size) {
    dbg_requires(size > 0);
    void *bp;
    size_t asize = elements * size;

    if (elements == 0) {
        return NULL;
    }
    if (asize / elements != size) {
        // Multiplication overflowed
        return NULL;
    }

    bp = malloc(asize);
    if (bp == NULL) {
        return NULL;
    }

    // Initialize all bits to 0
    memset(bp, 0, asize);

    block_t *blk = payload_to_header(bp);
    dbg_requires(get_alloc(blk));
    dbg_requires(get_payload_size(blk) >= size);

    return bp;
}

## Default Features
- `backend-wgpu` (always enabled by default)
- `widgets` (retained-mode widgets: Button, TextInput, Text2D)

## Optional Features

### `layout`
Simple layout helpers (anchors, percent sizing, margins). Useful for UI layouts.

**Enable in your project:**
```toml
[dependencies]
plutonium_engine = { path = "../path/to/plutonium_engine", features = ["layout"] }
# or if published to crates.io:
# plutonium_engine = { version = "0.7.0", features = ["layout"] }
```

### `raster`
PNG/JPEG image loading helpers via `create_texture_raster_from_path`. Supports responsive fitting (Contain, Cover, StretchFill) and logical insets for uniform padding. See `docs/textures.md` for details.

### `anim`
Tweening and animation helpers: `Tween`, `Track`, `Timeline` with CSS-like easing.

### `replay`
Deterministic RNG streams and record/replay helpers.

## Enabling Multiple Features

You can enable multiple optional features:

```toml
[dependencies]
plutonium_engine = { path = "../path/to/plutonium_engine", features = ["layout", "anim"] }
```

## Feature Combinations in Your Project

If you want to customize which features are enabled in your project, you can do so in your `Cargo.toml`:

```toml
[dependencies]
plutonium_engine = { path = "../../../" }

# Then enable specific features:
[dependencies.plutonium_engine]
features = ["layout", "anim"]
default-features = false  # Disable default features if you only want specific ones
```

Or more simply, just enable what you need:
```toml
[dependencies]
plutonium_engine = { path = "../../../", features = ["layout"] }
```
