"""Public API probes for rejected training and partially allocated batch rollback."""
import data_pipeline_programs as programs
import data_pipeline_oracles as oracle


def failures():
    return programs.common()+'''
use crate::tensor::model::Model;
use alloc::bytes::len;use alloc::bytes::new;use alloc::bytes::try_resize;use alloc::bytes::GrowStatus;
fn compare_state(model:&Model,state:&State,sampler:&Sampler,before:&[u8],sampling:&[u8])->i32{
 let after=crate::tensor::model::session::encode(&*model,&*state);match after{TensorResult::Ok(bytes)=>{
 if !same(&bytes,&*before){return 101;}let encoded=crate::tensor::data::sampler_checkpoint::encode(&*sampler);match encoded{
 TensorResult::Ok(output)=>{if !same(&output,&*sampling){return 102;}return 0;},TensorResult::Err(error)=>{return 103;}}},TensorResult::Err(error)=>{return 104;}}
}
fn reject(model:&mut Model,state:&mut State,data:&Dataset,sampler:&mut Sampler,wanted:i32)->i32{
 let before=crate::tensor::model::session::encode(&*model,&*state);match before{TensorResult::Ok(bytes)=>{
 let encoded=crate::tensor::data::sampler_checkpoint::encode(&*sampler);match encoded{TensorResult::Ok(sampling)=>{
 let applied=crate::tensor::data::train_next(&mut *model,&mut *state,&*data,&mut *sampler);match applied{
 TensorResult::Ok(value)=>{return 105;},TensorResult::Err(error)=>{if error_code(error)!=wanted{return 106;}}}
 return compare_state(&*model,&*state,&*sampler,&bytes,&sampling);
 },TensorResult::Err(error)=>{return 107;}}},TensorResult::Err(error)=>{return 108;}}
}
fn attempt(created:Model,data:&Dataset,wanted:i32)->i32{
 let mut model=created;let selected=crate::tensor::optimizer::sgd(0.05);match selected{TensorResult::Ok(setting)=>{
 let initialized=crate::tensor::optimizer::initialize(1,crate::tensor::model::parameter_elements(&model),setting);match initialized{TensorResult::Ok(ready)=>{
 let mut state=ready;let prepared=crate::tensor::data::initialize_sampler(&*data,1,true,1u32);match prepared{TensorResult::Ok(initial)=>{
 let mut sampler=initial;return reject(&mut model,&mut state,&*data,&mut sampler,wanted);
 },TensorResult::Err(error)=>{return 109;}}},TensorResult::Err(error)=>{return 110;}}},TensorResult::Err(error)=>{return 111;}}
}
fn checks(data:Dataset,features:i32,wanted:i32)->i32{
 let initialized=crate::tensor::model::initialize(features,2,1,42u32);match initialized{
 TensorResult::Ok(model)=>{return attempt(model,&data,wanted);},TensorResult::Err(error)=>{return 112;}}
}
fn rejected_update(features:i32,value:f64,target:f64,wanted:i32)->i32{
 let first=crate::tensor::filled(2,2,value);match first{TensorResult::Ok(input)=>{
 let second=crate::tensor::filled(2,1,target);match second{TensorResult::Ok(expected)=>{
 let made=crate::tensor::data::create(input,expected);match made{TensorResult::Ok(data)=>{return checks(data,features,wanted);},TensorResult::Err(error)=>{return 113;}}
 },TensorResult::Err(error)=>{return 114;}}},TensorResult::Err(error)=>{return 115;}}
}
fn memory_error(result:TensorResult<Batch>)->i32{match result{
 TensorResult::Ok(batch)=>{return 116;},TensorResult::Err(error)=>{if error_code(error)!=6{return 117;}return 0;}}}
fn allocation_probe()->i32{let allocated=crate::tensor::filled(1,131072,0.0);match allocated{
 TensorResult::Ok(value)=>{return 0;},TensorResult::Err(error)=>{return 118;}}}
fn memory_test(data:&Dataset,created:Sampler)->i32{
 let mut sampler=created;let mut held=new();let grown=try_resize(&mut held,13107200,0u8);match grown{
 GrowStatus::Ok=>{},GrowStatus::CapacityOverflow=>{return 119;},GrowStatus::AllocationFailed=>{return 120;},GrowStatus::MemoryLimit=>{return 121;}}
 for cycle in 0..3{
  let result=memory_error(crate::tensor::data::next_batch(&*data,&mut sampler));if result!=0{return result;}
  if crate::tensor::data::batch_count(&sampler)!=0u64||crate::tensor::data::position(&sampler)!=0{return 122;}
  let probe=allocation_probe();if probe!=0{return probe;}if len(&held)!=13107200{return 123;}
 }return 0;
}
fn memory_ready(data:Dataset)->i32{let initialized=crate::tensor::data::initialize_sampler(&data,1,false,1u32);match initialized{
 TensorResult::Ok(sampler)=>{return memory_test(&data,sampler);},TensorResult::Err(error)=>{return 124;}}}
fn memory_rollback()->i32{
 let first=crate::tensor::filled(1,131072,0.0);match first{TensorResult::Ok(input)=>{
 let second=crate::tensor::filled(1,131072,0.0);match second{TensorResult::Ok(expected)=>{
 let made=crate::tensor::data::create(input,expected);match made{TensorResult::Ok(data)=>{return memory_ready(data);},TensorResult::Err(error)=>{return 125;}}
 },TensorResult::Err(error)=>{return 126;}}},TensorResult::Err(error)=>{return 127;}}
}
fn constructor(rows:i32,columns:i32,other_rows:i32,other_columns:i32,number:f64,target:f64,wanted:i32)->i32{
 let first=crate::tensor::filled(rows,columns,number);match first{TensorResult::Ok(input)=>{
 let second=crate::tensor::filled(other_rows,other_columns,target);match second{TensorResult::Ok(expected)=>{
 let made=crate::tensor::data::create(input,expected);match made{
 TensorResult::Ok(data)=>{return 128;},TensorResult::Err(error)=>{if error_code(error)!=wanted{return 129;}return 0;}}
 },TensorResult::Err(error)=>{return 130;}}},TensorResult::Err(error)=>{return 131;}}
}
fn constructors()->i32{
 if constructor(0,1,0,1,0.0,0.0,1)!=0{return 132;}
 if constructor(1,0,1,1,0.0,0.0,1)!=0{return 133;}
 if constructor(2,1,3,1,0.0,0.0,3)!=0{return 134;}
 let nan_bits=9221120237041090560u64;let nan=f64::from_bits(nan_bits);
 if constructor(2,1,2,1,nan,0.0,11)!=0{return 135;}
 let inf_bits=9218868437227405312u64;let infinity=f64::from_bits(inf_bits);
 if constructor(2,1,2,1,0.0,infinity,11)!=0{return 136;}
 return 0;
}
fn main()->i32{
 let invalid=constructors();if invalid!=0{return invalid;}
 let shape=rejected_update(1,1.0,0.0,3);if shape!=0{return shape;}
 let nonfinite=rejected_update(2,1e308,-1e308,11);if nonfinite!=0{return nonfinite;}
 return memory_rollback();
}
'''


def maximum():
    state = oracle.sampler(7, 3, True, 1)
    state.update(batches=oracle.MAXIMUM, epoch=oracle.MAXIMUM//3, cursor=(oracle.MAXIMUM % 3)*3)
    literal = ','.join(str(byte)+'u8' for byte in oracle.encode_sampler(state))
    return programs.common()+'''
fn verify(data:Dataset,ready:Sampler,before:&[u8])->i32{
 let mut state=ready;let next=crate::tensor::data::next_batch(&data,&mut state);match next{
 TensorResult::Ok(batch)=>{return 101;},TensorResult::Err(error)=>{if error_code(error)!=14{return 102;}}}
 let encoded=crate::tensor::data::sampler_checkpoint::encode(&state);match encoded{
 TensorResult::Ok(bytes)=>{if !same(&bytes,&*before){return 103;}return 0;},TensorResult::Err(error)=>{return 104;}}
}
fn prepared(data:Dataset)->i32{let bytes=['''+literal+'''];let loaded=crate::tensor::data::sampler_checkpoint::decode(&bytes);match loaded{
 TensorResult::Ok(state)=>{return verify(data,state,&bytes);},TensorResult::Err(error)=>{return 105;}}}
fn main()->i32{let first=crate::tensor::filled(7,1,0.0);match first{TensorResult::Ok(input)=>{
 let second=crate::tensor::filled(7,1,0.0);match second{TensorResult::Ok(target)=>{
 let made=crate::tensor::data::create(input,target);match made{TensorResult::Ok(data)=>{return prepared(data);},TensorResult::Err(error)=>{return 106;}}
 },TensorResult::Err(error)=>{return 107;}}},TensorResult::Err(error)=>{return 108;}}}
'''
