"""Oxid programs for testing the public optimizer API without a numerical bridge."""
import optimizer_oracles as oracle


GRADIENTS = [[.3, -.2, .4], [-.1, .5, -.4], [1., 0., .01]]


def cases():
    result = []
    for name in ['sgd', 'adam', 'adamw']:
        result.append({'name': name, 'config': oracle.config(name), 'parameters': [1., -2., .5], 'gradients': GRADIENTS})
    for name in ['sgd', 'adam', 'adamw']:
        result.append({'name': name+'-clipped-scheduled', 'config': oracle.config(name, clip=.25, factor=.5, every=2),
                       'parameters': [1., -2., .5], 'gradients': GRADIENTS})
    result += [
        {'name': 'overflow-safe-clipping', 'config': oracle.config(rate=1., clip=1.), 'parameters': [0., 0., 0.],
         'gradients': [[1e308, -1e308, 1e308]]},
        {'name': 'subnormal-clipping', 'config': oracle.config(rate=1., clip=oracle.value(1)), 'parameters': [0., 0., 0.],
         'gradients': [[3., 4., 0.]]},
        {'name': 'underflowing-variance', 'config': oracle.config('adam'), 'parameters': [0., 0., 0.],
         'gradients': [[1e-200, -1e-200, 0.], [2e-200, 1e-200, 0.]]},
        {'name': 'atomic-sgd', 'config': oracle.config(rate=1e308), 'parameters': [1., -2., .5],
         'gradients': GRADIENTS, 'rejected': [.1, .1, 1e308], 'error': 11},
        {'name': 'atomic-adam', 'config': oracle.config('adam'), 'parameters': [1., -2., .5],
         'gradients': GRADIENTS, 'rejected': [.1, .1, 1e308], 'error': 11},
        {'name': 'shape-rollback', 'config': oracle.config(), 'parameters': [1., -2., .5],
         'gradients': GRADIENTS, 'rejected': [1., 2.], 'error': 3},
        {'name': 'nan-rollback', 'config': oracle.config('adamw'), 'parameters': [1., -2., .5],
         'gradients': GRADIENTS, 'rejected': [1., 2., float('nan')], 'error': 11},
        {'name': 'zero-rate', 'config': oracle.config(rate=0.), 'parameters': [0., -0., 1.],
         'gradients': [[1e308, 1e308, -1e308]]},
    ]
    setting = oracle.config('adam'); setting.update(beta1=0., beta2=0.)
    result.append({'name': 'zero-beta', 'config': setting, 'parameters': [1., -2., .5], 'gradients': GRADIENTS})
    setting = oracle.config('adam'); setting['epsilon'] = oracle.value(1)
    result.append({'name': 'subnormal-epsilon', 'config': setting, 'parameters': [0., 0., 0.],
                   'gradients': [[oracle.value(1), 0., oracle.value(1)]]})
    return result


ERRORS = ['NegativeShape', 'SizeOverflow', 'ShapeMismatch', 'Bounds', 'AllocationFailed', 'MemoryLimit',
          'InvalidStorage', 'EmptyReduction', 'InvalidNode', 'GraphCapacity', 'NonFinite', 'InvalidRate',
          'InvalidOptimizer', 'StepOverflow', 'InvalidCheckpoint', 'IoError(code)', 'WriteZero']


def common():
    arms = ''.join(f'Error::{name}=>{{return {index};}},' for index, name in enumerate(ERRORS, 1))
    return '''use crate::tensor::Tensor;use crate::tensor::TensorResult;use crate::tensor::Error;
use crate::tensor::optimizer::Config;use crate::tensor::optimizer::State;
use alloc::bytes::ByteBuf;use std::io::write_some;use std::io::WriteSomeStatus;
fn error_code(error:Error)->i32{match error{''' + arms + '''}}
fn status(result:TensorResult<()>)->i32{match result{TensorResult::Ok(done)=>{return 0;},TensorResult::Err(error)=>{return error_code(error);}}}
fn emit(bytes:&[u8])->i32{
 let mut offset=0;while offset<bytes.len(){let written=write_some(&*bytes[offset..bytes.len()]);
 match written{WriteSomeStatus::Written(count)=>{if count<=0{return 90;}offset=offset+count;},WriteSomeStatus::Error(code)=>{if code!=4{return 90;}}}}return 0;
}
fn same(first:&[u8],second:&[u8])->bool{if first.len()!=second.len(){return false;}for index in 0..first.len(){if first[index]!=second[index]{return false;}}return true;}
fn emit_parameters(parameter:&Tensor)->i32{
 let mut values=[0.0,0.0,0.0];if status(crate::tensor::copy_values(&*parameter,&mut values))!=0{return 91;}
 let mut bytes=[0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8,0u8];for element in 0..3{let number=values[element];let pattern=number.to_bits();
 for offset in 0..8{let amount=offset*8;let shift=amount.to_u64_checked();let part=(pattern>>shift)&255u64;bytes[element*8+offset]=part.to_u8_checked();}}
 return emit(&bytes);
}
fn finish(parameter:&Tensor,state:&State)->i32{
 if emit_parameters(&*parameter)!=0{return 92;}let saved=crate::tensor::optimizer::checkpoint::encode(&*state);
 match saved{TensorResult::Ok(bytes)=>{return emit(&bytes);},TensorResult::Err(error)=>{return 93;}}
}
'''


def numbers(values):
    return ''.join(f'let bits_{i}={oracle.bits(x)}u64;let number_{i}=f64::from_bits(bits_{i});' for i, x in enumerate(values))


def tensor(name, values):
    return (f'fn {name}()->TensorResult<Tensor>{{' + numbers(values)
            + 'let values=[' + ','.join(f'number_{i}' for i in range(len(values)))
            + f'];return crate::tensor::from_slice(1,{len(values)},&values);}}')


def configuration(settings):
    fields = ['rate', 'beta1', 'beta2', 'epsilon', 'decay', 'clip', 'factor']
    constants = numbers([settings[name] for name in fields])
    constructor = ['sgd(number_0)', 'adam(number_0,number_1,number_2,number_3)',
                   'adamw(number_0,number_1,number_2,number_3,number_4)'][settings['kind']]
    source = 'fn configuration()->TensorResult<Config>{' + constants
    source += 'let selected=crate::tensor::optimizer::' + constructor + ';match selected{TensorResult::Ok(value)=>{let mut setting=value;'
    source += '''let clipped=crate::tensor::optimizer::set_clip_norm(&mut setting,number_5);
match clipped{TensorResult::Ok(done)=>{},TensorResult::Err(error)=>{return TensorResult::<Config>::Err(error);}}'''
    if settings['every']:
        source += f'let scheduled=crate::tensor::optimizer::set_step_decay(&mut setting,number_6,{settings["every"]}u64);'
        source += 'match scheduled{TensorResult::Ok(done)=>{},TensorResult::Err(error)=>{return TensorResult::<Config>::Err(error);}}'
    return source + 'return TensorResult::Ok(setting);},TensorResult::Err(error)=>{return TensorResult::<Config>::Err(error);}}}'


def rejected_step(expected):
    return '''fn rejected_update(parameter:&mut Tensor,state:&mut State)->TensorResult<()>{
 let made=invalid_gradient();match made{TensorResult::Ok(gradient)=>{return crate::tensor::optimizer::step(&mut *state,&mut *parameter,&gradient);},TensorResult::Err(error)=>{return TensorResult::<()>::Err(error);}}
}
fn compare_failure(before:ByteBuf,parameter:&mut Tensor,state:&mut State)->i32{
 let mut old=[0.0,0.0,0.0];if status(crate::tensor::copy_values(&*parameter,&mut old))!=0{return 94;}
 if status(rejected_update(&mut *parameter,&mut *state))!=EXPECTED{return 95;}
 let mut after=[0.0,0.0,0.0];if status(crate::tensor::copy_values(&*parameter,&mut after))!=0{return 96;}
 for index in 0..3{let first=old[index];let second=after[index];if first.to_bits()!=second.to_bits(){return 97;}}
 let encoded=crate::tensor::optimizer::checkpoint::encode(&*state);
 match encoded{TensorResult::Ok(bytes)=>{if !same(&before,&bytes){return 98;}return 0;},TensorResult::Err(error)=>{return 99;}}
}
fn check_failure(parameter:&mut Tensor,state:&mut State)->i32{
 let saved=crate::tensor::optimizer::checkpoint::encode(&*state);
 match saved{TensorResult::Ok(before)=>{return compare_failure(before,&mut *parameter,&mut *state);},TensorResult::Err(error)=>{return 100;}}
}
'''.replace('EXPECTED', str(expected))


def generate(case):
    source = common() + configuration(case['config']) + tensor('initial_parameters', case['parameters'])
    for index, gradient in enumerate(case['gradients']):
        source += tensor(f'gradient_{index}', gradient)
    if 'rejected' in case:
        source += tensor('invalid_gradient', case['rejected']) + rejected_step(case['error'])
    source += 'fn execute(created:Tensor,created_state:State)->i32{let mut parameter=created;let mut state=created_state;'
    if 'rejected' in case:
        source += 'let tested=check_failure(&mut parameter,&mut state);if tested!=0{return tested;}'
    for index in range(len(case['gradients'])):
        source += f'let made_{index}=gradient_{index}();match made_{index}{{TensorResult::Ok(gradient)=>{{'
        source += f'if status(crate::tensor::optimizer::step(&mut state,&mut parameter,&gradient))!=0{{return {110+index};}}'
        source += '},TensorResult::Err(error)=>{return 114;}}'
    source += 'return finish(&parameter,&state);}'
    source += '''fn initialized(parameter:Tensor,setting:Config)->i32{
 let made=crate::tensor::optimizer::initialize(1,3,setting);match made{TensorResult::Ok(state)=>{return execute(parameter,state);},TensorResult::Err(error)=>{return 115;}}
}
fn configured(parameter:Tensor)->i32{let made=configuration();match made{TensorResult::Ok(setting)=>{return initialized(parameter,setting);},TensorResult::Err(error)=>{return 116;}}}
fn main()->i32{let made=initial_parameters();match made{TensorResult::Ok(parameter)=>{return configured(parameter);},TensorResult::Err(error)=>{return 117;}}}
'''
    return source
