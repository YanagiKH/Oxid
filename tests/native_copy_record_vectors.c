/* Independent literal schemas and byte/fuel/failure oracles for record vectors.
 * No Oxid producer, inferred C layout or runtime width query supplies expectations. */
#define _POSIX_C_SOURCE 200809L
#define OXID_BYTEBUF_TESTING
#define OXID_RECORD_VECTOR_SCHEMAS 1
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "../native/experimental_vector.h"
static const oxid_vector_record_leaf point_leaves[] = {
    {0,0,OXID_VECTOR_BOOL},{1,4,OXID_VECTOR_I32},
    {5,8,OXID_VECTOR_CHAR},{9,12,OXID_VECTOR_UNIT}
};
static const oxid_vector_record_leaf empty_leaves[] = {{0,0,OXID_VECTOR_UNIT}};
static const oxid_vector_record_leaf wide_leaves[] = {
    {0,0,OXID_VECTOR_BOOL},{1,16,OXID_VECTOR_U128},
    {17,32,OXID_VECTOR_CHAR},{21,36,OXID_VECTOR_UNIT}
};
static const oxid_vector_record_leaf invalid_leaves[] = {{0,0,OXID_VECTOR_BOOL},{0,1,OXID_VECTOR_UNIT}};
const uint32_t __oxid_record_vector_schema_count = 5;
const oxid_vector_record_schema __oxid_record_vector_schemas[] = {
    {UINT32_C(0x80000000),10,16,4,point_leaves},
    {UINT32_C(0x80000001),10,16,4,point_leaves},
    {UINT32_C(0x80000002),1,1,1,empty_leaves},
    {UINT32_C(0x80000003),22,48,4,wide_leaves},
    {UINT32_C(0x80000004),2,2,2,invalid_leaves}
};
static int force_null;
static void *record_allocate(size_t alignment, size_t length) {
    return force_null ? NULL : aligned_alloc(alignment,length);
}
#define OXID_VECTOR_ALIGNED_ALLOC record_allocate
#include "../native/typed_bytebuf.c"

#define POINT UINT32_C(0x80000000)
#define OTHER UINT32_C(0x80000001)
#define EMPTY UINT32_C(0x80000002)
#define WIDE UINT32_C(0x80000003)
static const uint8_t dense[10] = {1,21,0,0,0,255,255,16,0,0};
static uint64_t enough(void) { return UINT64_C(1000000); }
static void owner_bytes(uint8_t *owner) {
    memset(owner,0xa5,16);
    owner[0]=1;
    memcpy(owner+4,dense+1,4);
    memcpy(owner+8,dense+5,4);
    owner[12]=0;
}
static void expect_values(const oxid_vector *value, int32_t count) {
    uint8_t *bytes = NULL;
    assert(oxid_vector_range(&execution_heap,value,POINT,0,count,&bytes)==0);
    for(int32_t i=0;i<count;++i) assert(!memcmp(bytes+(size_t)i*10,dense,10));
}
static void all_operations_and_copy_padding(void) {
    __oxid_bytebuf_start();
    oxid_vector value,source,moved;
    assert(oxid_vector_new(POINT,&value)==0);
    assert(oxid_vector_new(POINT,&source)==0);
    uint64_t fuel=enough();
    int32_t length=-1,capacity=-1;
    assert(oxid_vector_len(&execution_heap,&value,POINT,&length)==0 && length==0);
    assert(oxid_vector_capacity(&execution_heap,&value,POINT,&capacity)==0 && capacity==0);
    assert(oxid_vector_reserve(&execution_heap,&value,POINT,2,&fuel)==0);
    assert(value.length==0 && value.capacity==8 && execution_heap.ledger.live_bytes==128);
    uint8_t owner[16],packed[10];
    owner_bytes(owner);
    memset(packed,0xcc,sizeof packed);
    assert(__oxid_record_vector_pack(POINT,owner,packed)==0 && !memcmp(packed,dense,10));
    assert(oxid_vector_push(&execution_heap,&value,POINT,packed,&fuel)==0);
    assert(oxid_vector_resize(&execution_heap,&source,POINT,3,packed,&fuel)==0);
    assert(oxid_vector_extend_vector(&execution_heap,&value,POINT,&source,1,3,&fuel)==0);
    assert(value.length==3);
    expect_values(&value,3);
    assert(oxid_vector_resize(&execution_heap,&value,POINT,9,packed,&fuel)==0);
    expect_values(&value,9);
    uint8_t *cell=NULL;
    assert(__oxid_vector_index(&value,POINT,8,&cell)==0);
    uint8_t copied[16]; memset(copied,0xcc,sizeof copied);
    assert(__oxid_record_vector_unpack(POINT,cell,copied)==0);
    const uint8_t expected_owner[16]={1,204,204,204,21,0,0,0,255,255,16,0,0,204,204,204};
    assert(!memcmp(copied,expected_owner,16));
    copied[4]=42;
    assert(__oxid_record_vector_pack(POINT,copied,packed)==0);
    assert(__oxid_record_vector_write(POINT,packed,cell)==0);
    assert(cell[1]==42 && owner[4]==21); /* independent Copy value */
    assert(oxid_vector_truncate(&execution_heap,&value,POINT,2)==0 && value.length==2);
    assert(oxid_vector_take(&execution_heap,&value,POINT,&moved)==0 && !value.identity && !value.length && !value.capacity);
    assert(moved.length==2);
    assert(oxid_vector_release(&execution_heap,&moved,POINT)==0);
    assert(oxid_vector_release(&execution_heap,&source,POINT)==0);
    assert(execution_heap.ledger.allocations==execution_heap.ledger.releases);
    assert(__oxid_bytebuf_finish()==0);
}
static void exact_fuel_and_peak_limits(void) {
    __oxid_bytebuf_start();
    execution_heap.limit=335;
    oxid_vector value={0,0,0};
    uint64_t fuel=1389;
    assert(oxid_vector_resize(&execution_heap,&value,POINT,8,dense,&fuel)==OXID_BUFFER_FUEL);
    assert(fuel==1389 && !value.identity && !execution_heap.ledger.live_bytes);
    fuel=1390;
    assert(oxid_vector_resize(&execution_heap,&value,POINT,8,dense,&fuel)==0 && !fuel);
    assert(execution_heap.ledger.live_bytes==128);
    oxid_vector before=value;
    fuel=80;
    assert(oxid_vector_reserve(&execution_heap,&value,POINT,1,&fuel)==OXID_MEMORY_LIMIT && fuel==80);
    assert(!memcmp(&value,&before,sizeof value));
    execution_heap.limit=336;
    fuel=79;
    assert(oxid_vector_reserve(&execution_heap,&value,POINT,1,&fuel)==OXID_BUFFER_FUEL && fuel==79);
    fuel=80;
    assert(oxid_vector_reserve(&execution_heap,&value,POINT,1,&fuel)==0 && !fuel);
    assert(execution_heap.ledger.live_bytes==208 && execution_heap.ledger.peak_bytes==336);
    expect_values(&value,8);
    assert(oxid_vector_release(&execution_heap,&value,POINT)==0);
    assert(__oxid_bytebuf_finish()==0);
}
static void canonical_nominal_and_extent_refusals_are_atomic(void) {
    __oxid_bytebuf_start();
    oxid_vector value={0,0,0},other={0,0,0};
    uint64_t fuel=enough();
    assert(oxid_vector_push(&execution_heap,&value,POINT,dense,&fuel)==0);
    assert(oxid_vector_push(&execution_heap,&other,OTHER,dense,&fuel)==0);
    assert(oxid_vector_validate(&execution_heap,&value,OTHER)==OXID_BUFFER_DESCRIPTOR);
    assert(oxid_vector_validate(&execution_heap,&value,OXID_VECTOR_U8)==OXID_BUFFER_DESCRIPTOR);
    assert(oxid_vector_new(UINT32_C(0x80001000),&(oxid_vector){0,0,0})==OXID_BUFFER_DESCRIPTOR);
    oxid_vector before=value;
    for(int kind=0;kind<4;++kind) {
        uint8_t invalid[10]; memcpy(invalid,dense,10);
        if(kind==0) invalid[0]=2;
        if(kind==1) invalid[9]=1;
        if(kind==2) { invalid[5]=0;invalid[6]=216;invalid[7]=0;invalid[8]=0; }
        if(kind==3) { invalid[5]=0;invalid[6]=0;invalid[7]=17;invalid[8]=0; }
        uint64_t budget=enough(),saved=budget;
        assert(oxid_vector_push(&execution_heap,&value,POINT,invalid,&budget)==OXID_BUFFER_DESCRIPTOR && budget==saved);
        assert(!memcmp(&value,&before,sizeof value));
        expect_values(&value,1);
    }
    uint8_t *point=NULL,*other_cell=NULL;
    assert(__oxid_vector_index(&value,POINT,0,&point)==0);
    assert(__oxid_vector_index(&other,OTHER,0,&other_cell)==0);
    assert(oxid_vector_extend(&execution_heap,&value,POINT,other_cell,1,&fuel)==OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(&value,&before,sizeof value));
    uint8_t owner[16],expected[16]; memset(owner,0xa5,16);memcpy(expected,owner,16);
    assert(__oxid_record_vector_unpack(POINT,other_cell,owner)==OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(owner,expected,16));
    uint8_t *selected=NULL;
    assert(__oxid_record_vector_index_slice(point,POINT,1,0,&selected)==0 && selected==point);
    assert(__oxid_record_vector_index_slice(point,POINT,1,1,&selected)==OXID_BUFFER_BOUNDS);
    assert(__oxid_record_vector_index_slice(other_cell,POINT,1,0,&selected)==OXID_BUFFER_DESCRIPTOR);
    uint8_t *raw=NULL;
    assert(oxid_vector_range(&execution_heap,&value,POINT,0,1,&raw)==0);
    raw[9]=1; /* corrupt the last leaf, after earlier valid bool/int/char leaves */
    assert(__oxid_record_vector_unpack(POINT,raw,owner)==OXID_BUFFER_DESCRIPTOR);
    assert(!memcmp(owner,expected,16));
    raw[9]=0;
    owner_bytes(owner); owner[12]=1;
    uint8_t packed[10];memset(packed,0xcc,10);
    assert(__oxid_record_vector_pack(POINT,owner,packed)==OXID_BUFFER_DESCRIPTOR);
    for(size_t i=0;i<10;++i) assert(packed[i]==0xcc);
    oxid_vector malformed={0,0,0};uint8_t invalid_schema[2]={1,0};
    assert(oxid_vector_push(&execution_heap,&malformed,UINT32_C(0x80000004),invalid_schema,&fuel)==OXID_BUFFER_DESCRIPTOR);
    assert(!malformed.identity);
    assert(oxid_vector_release(&execution_heap,&value,POINT)==0);
    assert(oxid_vector_release(&execution_heap,&other,OTHER)==0);
    assert(__oxid_bytebuf_finish()==0);
}
static void allocation_faults_leave_live_state_unchanged(void) {
    for(int phase=ALLOCATE;phase<=COMMIT;++phase) for(int initial=0;initial<=8;initial+=8) {
        __oxid_bytebuf_start();
        oxid_vector value={0,0,0};uint64_t fuel=enough();
        assert(oxid_vector_resize(&execution_heap,&value,POINT,initial,dense,&fuel)==0);
        oxid_vector before=value;
        uint64_t live=execution_heap.ledger.live_bytes,allocations=execution_heap.ledger.live_allocations;
        execution_heap.fault_boundary=phase;execution_heap.fault_skip=0;
        assert(oxid_vector_resize(&execution_heap,&value,POINT,9,dense,&fuel)==OXID_ALLOCATION_FAILED);
        assert(!memcmp(&value,&before,sizeof value));
        assert(execution_heap.ledger.live_bytes==live && execution_heap.ledger.live_allocations==allocations);
        expect_values(&value,initial);
        assert(oxid_vector_resize(&execution_heap,&value,POINT,9,dense,&fuel)==0);
        expect_values(&value,9);
        assert(oxid_vector_release(&execution_heap,&value,POINT)==0);
        assert(__oxid_bytebuf_finish()==0);
    }
    __oxid_bytebuf_start();
    oxid_vector value={0,0,0};uint64_t fuel=enough();force_null=1;
    assert(oxid_vector_push(&execution_heap,&value,POINT,dense,&fuel)==OXID_ALLOCATION_FAILED);
    force_null=0;assert(!value.identity && !execution_heap.ledger.live_bytes);
    assert(__oxid_bytebuf_finish()==0);
}
static void empty_and_wider_than_scalar_payloads(void) {
    __oxid_bytebuf_start();
    oxid_vector empty={0,0,0},wide={0,0,0};uint64_t fuel=132;uint8_t unit=0;
    assert(oxid_vector_push(&execution_heap,&empty,EMPTY,&unit,&fuel)==0 && !fuel);
    assert(execution_heap.ledger.live_bytes==64);
    uint8_t owner[48],packed[22];memset(owner,0xa5,48);owner[0]=1;
    memset(owner+16,255,16);owner[32]=255;owner[33]=255;owner[34]=16;owner[35]=0;owner[36]=0;
    assert(__oxid_record_vector_pack(WIDE,owner,packed)==0);
    assert(packed[0]==1 && packed[17]==255 && packed[18]==255 && packed[19]==16 && !packed[20] && !packed[21]);
    for(size_t i=1;i<=16;++i) assert(packed[i]==255);
    fuel=enough();assert(oxid_vector_push(&execution_heap,&wide,WIDE,packed,&fuel)==0);
    assert(execution_heap.ledger.live_bytes==64+224);
    uint8_t *cell=NULL;assert(__oxid_vector_index(&wide,WIDE,0,&cell)==0);
    uint8_t copy[48];memset(copy,0xcc,48);
    assert(__oxid_record_vector_unpack(WIDE,cell,copy)==0);
    for(size_t i=1;i<16;++i) assert(copy[i]==0xcc);
    for(size_t i=37;i<48;++i) assert(copy[i]==0xcc);
    assert(oxid_vector_release(&execution_heap,&wide,WIDE)==0);
    assert(oxid_vector_release(&execution_heap,&empty,EMPTY)==0);
    assert(__oxid_bytebuf_finish()==0);
}
int main(void) {
    _Static_assert(sizeof(oxid_vector)==16,"descriptor remains16 bytes");
    _Static_assert(sizeof(oxid_vector_header)==48,"header remains48 bytes");
    _Static_assert(sizeof(oxid_vector_record_leaf)==12,"schema leaf bytes");
    _Static_assert(sizeof(oxid_vector_record_schema)==24,"schema row bytes");
    all_operations_and_copy_padding();
    exact_fuel_and_peak_limits();
    canonical_nominal_and_extent_refusals_are_atomic();
    allocation_faults_leave_live_state_unchanged();
    empty_and_wider_than_scalar_payloads();
    return 0;
}
