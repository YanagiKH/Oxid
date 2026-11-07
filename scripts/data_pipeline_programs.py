"""Public Oxid probes for paired rows, deterministic epochs and sampler recovery."""
import optimizer_programs
import optimizer_checkpoint_programs


def tensor(name, rows, columns, values):
    return (f'fn {name}()->TensorResult<Tensor>{{'+optimizer_programs.numbers(values)
            + 'let values=['+','.join(f'number_{index}' for index in range(len(values)))+'];'
            + f'return crate::tensor::from_slice({rows},{columns},&values);}}')


def common():
    return optimizer_programs.common()+'''
use crate::tensor::data::Dataset;use crate::tensor::data::Sampler;use crate::tensor::data::Batch;
fn emit_tensor(value:&Tensor)->i32{
 let rows=crate::tensor::row_count(&*value);let columns=crate::tensor::column_count(&*value);
 for row in 0..rows{for column in 0..columns{let read=crate::tensor::get(&*value,row,column);match read{
 TensorResult::Ok(number)=>{let bits=number.to_bits();let mut bytes=[0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8];
 for offset in 0..8{let part=(bits>>(offset*8))&255u64;bytes[offset]=part.to_u8_checked();}if emit(&bytes)!=0{return 95;}},
 TensorResult::Err(error)=>{return error_code(error);}}}}return 0;
}
fn emit_sampler(state:&Sampler)->i32{let encoded=crate::tensor::data::sampler_checkpoint::encode(&*state);
 match encoded{TensorResult::Ok(bytes)=>{return emit(&bytes);},TensorResult::Err(error)=>{return error_code(error);}}}
'''


def sampling(case):
    rows = case['rows']
    inputs = [number for row in range(rows) for number in (float(row), -float(row))]
    targets = [number for row in range(rows) for number in (3.*row+.25, -.5*row)]
    return common()+tensor('inputs', rows, 2, inputs)+tensor('targets', rows, 2, targets)+'''
fn source()->TensorResult<Dataset>{let first=inputs();match first{TensorResult::Ok(input)=>{
 let second=targets();match second{TensorResult::Ok(expected)=>{return crate::tensor::data::create(input,expected);},TensorResult::Err(error)=>{return TensorResult::<Dataset>::Err(error);}}},TensorResult::Err(error)=>{return TensorResult::<Dataset>::Err(error);}}}
fn sample(data:&Dataset,created:Sampler)->i32{let mut state=created;
 if emit_sampler(&state)!=0{return 96;}
 for step in 0..'''+str(case['batches'])+'''{
  let before=crate::tensor::data::sampler_checkpoint::encode(&state);match before{TensorResult::Ok(bytes)=>{
   let restored=crate::tensor::data::sampler_checkpoint::decode(&bytes);match restored{TensorResult::Ok(ready)=>{state=ready;},TensorResult::Err(error)=>{return error_code(error);}}
  },TensorResult::Err(error)=>{return error_code(error);}}
  let next=crate::tensor::data::next_batch(&*data,&mut state);match next{TensorResult::Ok(batch)=>{
   let Batch{inputs:first,targets:second}=batch;if emit_tensor(&first)!=0||emit_tensor(&second)!=0{return 97;}
  },TensorResult::Err(error)=>{return error_code(error);}}
  if emit_sampler(&state)!=0{return 98;}
 }return 0;
}
fn run(data:Dataset)->i32{
 let serialized=crate::tensor::data::checkpoint::encode(&data);match serialized{TensorResult::Ok(bytes)=>{if emit(&bytes)!=0{return 99;}},TensorResult::Err(error)=>{return error_code(error);}}
 let made=crate::tensor::data::initialize_sampler(&data,'''+str(case['size'])+','+str(case['shuffle']).lower()+','+str(case['seed'])+'''u32);
 match made{TensorResult::Ok(state)=>{return sample(&data,state);},TensorResult::Err(error)=>{return error_code(error);}}
}
fn main()->i32{let made=source();match made{TensorResult::Ok(data)=>{return run(data);},TensorResult::Err(error)=>{return error_code(error);}}}
'''


def codec(kind):
    prefix = optimizer_checkpoint_programs.codec().split('fn overflow_attempt', 1)[0]
    if kind == 'session':
        return prefix+'''
use crate::tensor::data::session::Restored;
fn decode(bytes:&[u8])->i32{let loaded=crate::tensor::data::session::decode(&*bytes);match loaded{TensorResult::Ok(value)=>{
 let Restored{model:model,optimizer:state,dataset:data,sampler:sampler}=value;
 let saved=crate::tensor::data::session::encode(&model,&state,&data,&sampler);match saved{TensorResult::Ok(output)=>{return emit(&output);},TensorResult::Err(error)=>{return error_code(error);}}
},TensorResult::Err(error)=>{return error_code(error);}}}
fn main()->i32{let read=read_input();match read{TensorResult::Ok(bytes)=>{return decode(&bytes);},TensorResult::Err(error)=>{return error_code(error);}}}
'''
    if kind == 'dataset':
        module = 'crate::tensor::data::checkpoint'
    elif kind == 'sampler':
        module = 'crate::tensor::data::sampler_checkpoint'
    else:
        raise ValueError(kind)
    return prefix+f'''
fn decode(bytes:&[u8])->i32{{let loaded={module}::decode(&*bytes);match loaded{{TensorResult::Ok(value)=>{{
 let saved={module}::encode(&value);match saved{{TensorResult::Ok(output)=>{{return emit(&output);}},TensorResult::Err(error)=>{{return error_code(error);}}}}
}},TensorResult::Err(error)=>{{return error_code(error);}}}}}}
fn main()->i32{{let read=read_input();match read{{TensorResult::Ok(bytes)=>{{return decode(&bytes);}},TensorResult::Err(error)=>{{return error_code(error);}}}}}}
'''
