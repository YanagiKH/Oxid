"""Independent forward-only numerical and checkpoint oracles for CPU training."""
import math
import struct
import zlib


def report_program():
    patterns = [0, 1 << 63, 1, (1 << 63) | 1, 0x0010000000000000,
                0x7fefffffffffffff, 0xffefffffffffffff,
                0x7ff0000000000000, 0xfff0000000000000, 0x7ff8000000000001]
    patterns += [struct.unpack('<Q', struct.pack('<d', value))[0]
                 for value in (1.0, -1.0, .1, -1e-308, 1e308, 9.999995,
                               9.999994, 123456.78, 5.911256778307743e-30)]
    functions = []
    for index in range(0, len(patterns), 4):
        body = ['let updates=18446744073709551615u64;let path=b"checkpoint";']
        for offset, pattern in enumerate(patterns[index:index + 4]):
            body.append(f'let bits_{offset}={pattern}u64;let value_{offset}=f64::from_bits(bits_{offset});')
            body.append(f'if crate::report::summary(updates,value_{offset},&path)!=0{{return 1;}}')
        functions.append('fn numbers_' + str(index) + '()->i32{' + ''.join(body) + 'return 0;}')
    entry = ''.join('if numbers_' + str(index) + '()!=0{return 1;}' for index in range(0, len(patterns), 4))
    return 'mod report;\n' + ''.join(functions) + 'fn main()->i32{' + entry + 'return 0;}', patterns


def verify_report_output(body, patterns):
    lines = body.decode('ascii').splitlines()
    if len(lines) != 3 * len(patterns):
        raise ValueError('numeric report line count changed')
    for index, pattern in enumerate(patterns):
        group = lines[3 * index:3 * index + 3]
        if group[0] != 'Updates: 18446744073709551615' or group[2] != 'Checkpoint: checkpoint' or not group[1].startswith('MSE: '):
            raise ValueError('numeric report fields changed')
        value = struct.unpack('<d', struct.pack('<Q', pattern))[0]
        printed = float(group[1][5:])
        if math.isnan(value):
            if not math.isnan(printed):
                raise ValueError('NaN was not reported')
        elif math.isinf(value):
            if printed != value:
                raise ValueError('infinity sign changed')
        elif value == 0:
            if printed != 0 or math.copysign(1.0, printed) != math.copysign(1.0, value):
                raise ValueError('zero sign changed')
        elif not math.isfinite(printed) or abs(printed-value) > 5.1e-6 * abs(value):
            raise ValueError('six-significant-digit report is inaccurate')


def atomic_checkpoint():
    values = [0.0] * 33
    values[0] = 1.0
    values[24] = 1e-308
    body = b'OXMD0001' + struct.pack('<IIIIQ', 2, 8, 1, 1, 0)
    body += struct.pack('<' + 'd' * len(values), *values)
    return body + struct.pack('<I', zlib.crc32(body))


def atomic_update_program():
    return '''use crate::tensor::TensorResult;
use crate::tensor::Error;
use crate::tensor::Tensor;
use crate::tensor::model::Model;
fn nonfinite(error:Error)->bool{
 match error{
  Error::NonFinite=>{return true;},
  Error::NegativeShape=>{return false;},Error::SizeOverflow=>{return false;},
  Error::ShapeMismatch=>{return false;},Error::Bounds=>{return false;},
  Error::AllocationFailed=>{return false;},Error::MemoryLimit=>{return false;},
  Error::InvalidStorage=>{return false;},Error::EmptyReduction=>{return false;},
  Error::InvalidNode=>{return false;},Error::GraphCapacity=>{return false;},
  Error::InvalidRate=>{return false;},Error::InvalidOptimizer=>{return false;},Error::StepOverflow=>{return false;},
  Error::InvalidCheckpoint=>{return false;},Error::IoError(code)=>{return false;},
  Error::WriteZero=>{return false;}
 }
}
fn attempt(model:&mut Model,input:&Tensor,target:&Tensor)->i32{
 let changed=crate::tensor::model::train_step(&mut *model,&*input,&*target,1e308);
 match changed{TensorResult::Ok(number)=>{return 1;},TensorResult::Err(error)=>{if !nonfinite(error){return 2;}}}
 let path=b"after.bin";
 let stored=crate::checkpoint::save(&path,&*model);
 match stored{TensorResult::Ok(done)=>{return 0;},TensorResult::Err(error)=>{return 3;}}
}
fn check_model(created:Model)->i32{
 let mut model=created;
 let values=[1.0,1.0];let expected=[10.0];
 let input=crate::tensor::from_slice(1,2,&values);
 match input{TensorResult::Ok(data)=>{
  let target=crate::tensor::from_slice(1,1,&expected);
  match target{TensorResult::Ok(wanted)=>{return attempt(&mut model,&data,&wanted);},TensorResult::Err(error)=>{return 4;}}
 },TensorResult::Err(error)=>{return 5;}}
}
fn main()->i32{
 let path=b"before.bin";let loaded=crate::checkpoint::load(&path);
 match loaded{TensorResult::Ok(model)=>{return check_model(model);},TensorResult::Err(error)=>{return 6;}}
}
'''


def matrix(a,b,m,k,n):return [sum(a[i*k+t]*b[t*n+j]for t in range(k))for i in range(m)for j in range(n)]


def average(a):return sum(a)/len(a)


def square_model(a):return average([x*x for x in a])


def mat_model(a,b):return average(matrix(a,b,2,3,2))


def branched(a):return average([x*x+2*x for x in a])


def mlp_model(a,w,b,t):
 z=matrix(a,w,2,2,2);p=[max(0.0,x+b[i%2])for i,x in enumerate(z)]
 return average([(x-y)**2 for x,y in zip(p,t)])


cases=[
 {'name':'square-alias','data':[('a',1,2,[1.5,-2.0])],'ops':[('s','multiply','a','a'),('root','mean','s')],'function':square_model,'gradients':['a']},
 {'name':'matmul-mean','data':[('a',2,3,[-1.0,0.2,1.5,0.7,-0.9,0.3]),('b',3,2,[0.5,-0.6,1.2,0.8,-0.4,1.1])],'ops':[('p','matmul','a','b'),('root','mean','p')],'function':mat_model,'gradients':['a','b']},
 {'name':'branched-alias','data':[('a',1,3,[-0.5,0.7,2.0])],'ops':[('s','multiply','a','a'),('d','add','a','a'),('v','add','s','d'),('root','mean','v')],'function':branched,'gradients':['a']},
 {'name':'matrix-bias-relu-mse','data':[('a',2,2,[0.2,-0.4,1.5,0.7]),('w',2,2,[-0.3,0.8,0.6,-1.1]),('b',1,2,[0.25,-0.15]),('t',2,2,[0.1,-0.3,0.5,0.4])],'ops':[('z','matmul','a','w'),('h','add_rows','z','b'),('p','relu','h'),('root','mse','p','t')],'function':mlp_model,'gradients':['a','w','b','t']},
 {'name':'unreachable-nan','data':[('a',1,2,[1.5,-2.0]),('n',1,2,[float('nan'),float('nan')])],'ops':[('unused','multiply','a','n'),('root','mean','a')],'function':lambda a,n:average(a),'gradients':['a']},
]


def number(x):
 if math.isnan(x):return '(0f64/0f64)'
 return repr(float(x))


def literal(xs):return '['+','.join(number(x)for x in xs)+']'


def finite_gradient(fn,args,which):
 result=[]
 for index,value in enumerate(args[which]):
  plus=[list(x)for x in args];minus=[list(x)for x in args];eps=1e-6*max(1,abs(value));plus[which][index]+=eps;minus[which][index]-=eps
  result.append((fn(*plus)-fn(*minus))/(2*eps))
 return result


helpers = """
use crate::tensor::TensorResult;
use crate::tensor::graph::Graph;
use crate::tensor::graph::Variable;
fn make_input(tape: &mut Graph, rows:i32, columns:i32, data:&[f64])->TensorResult<Variable>{
 let made=crate::tensor::from_slice(rows,columns,&*data);
 match made { TensorResult::Ok(tensor)=>{return crate::tensor::graph::input(&mut *tape,&tensor);},TensorResult::Err(error)=>{return TensorResult::<Variable>::Err(error);} }
}
fn check_tensor(tensor:&crate::tensor::Tensor,expected:&[f64])->i32{
 let columns=crate::tensor::column_count(&*tensor);
 let rows=crate::tensor::row_count(&*tensor);
 if rows*columns!=expected.len(){return 90;}
 for element in 0..expected.len(){
  let read=crate::tensor::get(&*tensor,element/columns,element%columns);
  match read {TensorResult::Ok(number)=>{let delta=number-expected[element];if number!=number||delta>0.00001||delta< -0.00001{return element+1;}},TensorResult::Err(error)=>{return 91;}}
 }
 return 0;
}
fn check_loss(tape:&Graph,variable:&Variable,expected:f64)->i32{
 let values=crate::tensor::graph::values(&*tape,&*variable);
 match values {TensorResult::Ok(tensor)=>{let wanted=[expected];return check_tensor(&tensor,&wanted);},TensorResult::Err(error)=>{return 92;}}
}
fn check_gradient(tape:&Graph,variable:&Variable,expected:&[f64])->i32{
 let values=crate::tensor::graph::gradient(&*tape,&*variable);
 match values {TensorResult::Ok(tensor)=>{return check_tensor(&tensor,&*expected);},TensorResult::Err(error)=>{return 93;}}
}
"""


def generate(case,mutate=False):
 data=case['data'];args=[x[3]for x in data];expected_loss=case['function'](*args)
 gradients={name:finite_gradient(case['function'],args,next(i for i,d in enumerate(data)if d[0]==name))for name in case['gradients']}
 if mutate:gradients[case['gradients'][0]][0]+=1.0
 bindings=[];prefix=[];suffix=[]
 for name,rows,cols,values in data:
  prefix.append(f'let data_{name}={literal(values)};')
  bindings.append((f'make_input(&mut tape,{rows},{cols},&data_{name})',f'var_{name}'))
 for op in case['ops']:
  name,method,*parents=op
  expression='crate::tensor::graph::'+method+'(&mut tape,'+','.join('&var_'+p for p in parents)+')'
  bindings.append((expression,'var_'+name))
 bindings.append(('crate::tensor::graph::backward(&mut tape,&var_root)','backward_done'))
 suffix.append(f'let loss_check=check_loss(&tape,&var_root,{number(expected_loss)});if loss_check!=0{{return 10+loss_check;}}')
 for group,name in enumerate(case['gradients']):
  expected=gradients[name];prefix.append(f'let expected_{name}={literal(expected)};')
  suffix.append(f'let checked_{name}=check_gradient(&tape,&var_{name},&expected_{name});if checked_{name}!=0{{return {100*(group+1)}+checked_{name};}}')
 body=''.join(suffix)+'return 0;'
 for i,(expression,name)in reversed(list(enumerate(bindings))):body=f'let result_{i}={expression};match result_{i}{{TensorResult::Ok({name})=>{{{body}}},TensorResult::Err(error_{i})=>{{return {1000+i};}}}}'
 program=helpers+'fn main()->i32{let mut tape=crate::tensor::graph::create();'+''.join(prefix)+body+'}\n'
 return program,{'loss':expected_loss,'gradients':gradients}


def parse_checkpoint(body):
    if body[:8] != b'OXMD0001' or len(body) < 36:
        raise ValueError('invalid checkpoint header')
    inputs, hidden, outputs, rng, updates = struct.unpack_from('<IIIIQ', body, 8)
    count = inputs * hidden + hidden + hidden * outputs + outputs
    if min(inputs, hidden, outputs) <= 0 or not rng or count > 268435451:
        raise ValueError('invalid checkpoint dimensions or RNG')
    if len(body) != 36 + 8 * count or zlib.crc32(body[:-4]) != struct.unpack_from('<I', body, len(body) - 4)[0]:
        raise ValueError('invalid checkpoint extent or checksum')
    values = list(struct.unpack_from('<' + 'd' * count, body, 32))
    if not all(math.isfinite(x) for x in values):
        raise ValueError('non-finite checkpoint parameter')
    return {'inputs': inputs, 'hidden': hidden, 'outputs': outputs, 'rng': rng,
            'updates': updates, 'parameters': values}


def initial_parameters():
    state=42;values=[]
    def uniform(fan):
        nonlocal state
        state^=(state<<13)&0xffffffff;state^=state>>17;state^=(state<<5)&0xffffffff
        scale=1.0
        for _ in range(32):scale=.5*(scale+(6.0/fan)/scale)
        return ((state>>8)/16777216.0*2.0-1.0)*scale
    values.extend(uniform(2)for _ in range(16));values.extend([0.0]*8)
    values.extend(uniform(8)for _ in range(8));values.append(0.0)
    return values,state


def forward_loss(parameters):
    first=parameters[:16];bias=parameters[16:24];second=parameters[24:32];last=parameters[32]
    total=0.0
    for (a,b),target in zip([(0.,0.),(0.,1.),(1.,0.),(1.,1.)],[0.,1.,1.,0.]):
        hidden=[max(0.0,a*first[j]+b*first[8+j]+bias[j])for j in range(8)]
        prediction=sum(x*w for x,w in zip(hidden,second))+last
        total+=(prediction-target)**2
    return total/4


def finite_difference_step(parameters):
    gradients=[]
    for index,value in enumerate(parameters):
        plus=parameters[:];minus=parameters[:];epsilon=1e-6*max(1,abs(value));plus[index]+=epsilon;minus[index]-=epsilon
        gradients.append((forward_loss(plus)-forward_loss(minus))/(2*epsilon))
    return [v-.05*g for v,g in zip(parameters,gradients)]
