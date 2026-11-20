/* Standalone authored control for the private checked local-loan cell lookup. */
#define OXID_OWNED_VECTOR_RUNTIME 1
#define OXID_OWNED_VECTOR_SCHEMAS 1
#define OXID_BYTEBUF_TESTING 1
#include "experimental_bytebuf.c"
#include "experimental_owned_vector.c"
#include <assert.h>
const uint32_t __oxid_owned_type_count=5, __oxid_owned_field_count=3;
const uint32_t __oxid_owned_variant_count=0, __oxid_owned_record_count=2;
const oxid_owned_type __oxid_owned_types[]={
    {OXID_OWNED_SCALAR,OXID_VECTOR_BOOL,0,0,1},
    {OXID_OWNED_BYTEBUF,0,0,0,16},
    {OXID_OWNED_SCALAR,OXID_VECTOR_I32,0,0,4},
    {OXID_OWNED_RECORD,0,0,3,32},
    {OXID_OWNED_RECORD,1,0,3,32}
};
const oxid_owned_field __oxid_owned_fields[]={{0,0},{1,8},{2,24}};
const oxid_owned_field __oxid_owned_variants[]={{0,0}};
const oxid_owned_record_schema __oxid_owned_records[]={{0,3,32,7,1},{1,4,32,7,1}};
int main(void) {
    oxid_byte_heap heap;oxid_heap_init(&heap);
    oxid_owned_vector vector={0};uint64_t fuel=UINT64_MAX;
    assert(oxid_owned_vector_new(0,&vector)==0);
    assert(oxid_owned_vector_reserve(&heap,&vector,0,2,&fuel)==0);
    for(int32_t i=0;i<2;++i) {
        oxid_bytebuf child={0};assert(oxid_bytebuf_push(&heap,&child,(uint8_t)(41+i),&fuel)==0);
        uint8_t bytes[32]={1};memcpy(bytes+8,&child,16);int32_t number=41+i;memcpy(bytes+24,&number,4);
        assert(oxid_owned_vector_push_reserved(&heap,&vector,0,bytes,32,&fuel)==0);
        for(size_t j=0;j<32;++j)assert(bytes[j]==0);
    }
    oxid_heap_ledger before=heap.ledger;oxid_owned_vector encoded=vector;
    uint64_t work;assert(owned_vector_work(&heap,&__oxid_owned_records[0],2,0,&work));
    uint64_t exact=1+3+3+4*heap.ledger.live_allocations+work;
    uint8_t *cell=(uint8_t *)(uintptr_t)17;fuel=exact-1;
    assert(oxid_owned_vector_borrow_record(&heap,&vector,0,1,&fuel,&cell)==OXID_BUFFER_FUEL);
    assert(fuel==exact-1 && cell==(uint8_t *)(uintptr_t)17);
    heap.fault_boundary=ALLOCATE;heap.fault_skip=0;fuel=exact;
    assert(oxid_owned_vector_borrow_record(&heap,&vector,0,1,&fuel,&cell)==0);
    assert(fuel==0 && heap.fault_boundary==ALLOCATE);
    int32_t number;memcpy(&number,cell+24,4);assert(number==42);
    assert(memcmp(&before,&heap.ledger,sizeof before)==0 && memcmp(&encoded,&vector,sizeof vector)==0);
    heap.fault_boundary=0;
    uint8_t *first=NULL;fuel=UINT64_MAX;
    assert(oxid_owned_vector_borrow_record(&heap,&vector,0,0,&fuel,&first)==0);
    uint8_t saved_bool=cell[0];cell[0]=2;fuel=UINT64_MAX;uint8_t *result=(uint8_t *)(uintptr_t)17;
    assert(oxid_owned_vector_borrow_record(&heap,&vector,0,0,&fuel,&result)==OXID_BUFFER_DESCRIPTOR);
    assert(fuel==UINT64_MAX && result==(uint8_t *)(uintptr_t)17);cell[0]=saved_bool;
    uint8_t saved_child[16];memcpy(saved_child,cell+8,16);memcpy(cell+8,first+8,16);
    assert(oxid_owned_vector_borrow_record(&heap,&vector,0,0,&fuel,&result)==OXID_BUFFER_DESCRIPTOR);
    assert(fuel==UINT64_MAX && result==(uint8_t *)(uintptr_t)17);memcpy(cell+8,saved_child,16);
    assert(oxid_owned_vector_borrow_record(&heap,&vector,1,0,&fuel,&result)==OXID_BUFFER_DESCRIPTOR);
    assert(oxid_owned_vector_borrow_record(&heap,&vector,UINT32_C(0x80000000),0,&fuel,&result)==OXID_BUFFER_DESCRIPTOR);
    assert(oxid_owned_vector_borrow_record(&heap,&vector,0,-1,&fuel,&result)==OXID_BUFFER_BOUNDS);
    assert(oxid_owned_vector_borrow_record(&heap,&vector,0,2,&fuel,&result)==OXID_BUFFER_BOUNDS);
    assert(oxid_owned_vector_borrow_record(&heap,&vector,0,0,&fuel,(uint8_t **)(void *)(cell+8))==OXID_BUFFER_DESCRIPTOR);
    assert(fuel==UINT64_MAX && result==(uint8_t *)(uintptr_t)17);
    assert(memcmp(&before,&heap.ledger,sizeof before)==0);
    assert(oxid_owned_vector_release(&heap,&vector,0)==0);
    assert(heap.ledger.live_allocations==0 && heap.ledger.allocations==heap.ledger.releases);
    assert(oxid_owned_vector_borrow_record(&heap,&encoded,0,0,&fuel,&result)==OXID_BUFFER_DESCRIPTOR);
    return 0;
}
