const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const dir = __dirname;
const original = JSON.parse(fs.readFileSync(path.join(dir,'original-pixels.json'),'utf8').replace(/^\uFEFF/, '')).data;
const legend = {...original.legend}; delete legend['.'];
const project = 'f6286130';
const steps=[];
function step(tool,args){ steps.push({tool,args:{project,sprite:1,...args}}); }
const angles = [-78,-62,-32,8,47,73,68,46,14,-18,-47,-69];
const bob = [1,1,0,-1,-2,-2,-1,0,1,2,2,1];
const outline='#040b10', dark='#25353f', teal='#3b848b', cream='#f8efe6', gray='#e2dbd3', purple='#613858';
function wing(angle, root, scale, folded, far){
  const a=angle*Math.PI/180, c=Math.cos(a),s=Math.sin(a);
  function p([u,v]){return [Math.round(root[0]+scale*(u*c-v*s)),Math.round(root[1]+scale*(u*s+v*c*folded))];}
  const poly=(pts,color)=>({op:'polygon',points:pts.map(p),color});
  return [
    {op:'clear'},
    poly([[0,-3],[9,-6],[20,-6],[29,-4],[34,-2],[34,0],[28,2],[33,4],[32,6],[26,6],[30,9],[29,11],[23,10],[25,14],[23,16],[17,13],[18,17],[15,18],[8,12],[-2,6]],outline),
    poly([[2,-2],[10,-4],[21,-4],[30,-2],[26,1],[29,4],[24,5],[26,9],[21,8],[22,12],[17,10],[15,14],[9,10],[0,4]],dark),
    poly([[0,-2],[8,-4],[16,-3],[18,0],[14,5],[11,8],[6,8],[-1,4]],far?dark:teal),
    poly([[1,-2],[8,-4],[14,-3],[12,0],[6,1],[0,3]],far?purple:purple),
    poly([[15,-3],[20,-3],[21,0],[17,5],[13,9],[10,8],[13,4]],far?gray:cream),
    poly([[17,4],[20,1],[24,1],[22,4],[19,6]],far?dark:gray),
  ];
}
step('transaction_begin',{label:'Quero-quero: ciclo de voo'});
for(let i=0;i<12;i++){
  const frame=6+i, by=bob[i];
  // Nearest-neighbour inverse mapping preserves the imported sprite palette.
  const a=(-28+[0,0,0,-1,-2,-2,-1,0,1,1,0,0][i])*Math.PI/180,c=Math.cos(a),s=Math.sin(a);
  const rows=[];
  for(let y=0;y<80;y++){
    let row='';
    for(let x=0;x<80;x++){
      const dx=x-40,dy=y-38-by;
      const sx=Math.round((dx*c+dy*s)/1.05+25), sy=Math.round((-dx*s+dy*c)/.92+32);
      let ch=(sx>=0&&sx<50&&sy>=0&&sy<48)?original.rows[sy][sx]:'.';
      if(sy===47&&sx>=18&&sx<=26)ch='a';
      row+=ch;
    }
    rows.push(row);
  }
  const shift=pts=>pts.map(([x,y])=>[x,y+by]);
  const body=[{op:'clear'},
    {op:'polyline',points:shift([[42,48],[48,52],[57,50],[61,51]]),color:outline,thickness:3},
    {op:'polyline',points:shift([[42,48],[48,52],[57,50],[61,51]]),color:'#ef7648',thickness:1},
    {op:'polyline',points:shift([[47,47],[52,49],[61,47],[64,48]]),color:outline,thickness:3},
    {op:'polyline',points:shift([[47,47],[52,49],[61,47],[64,48]]),color:'#f59e52',thickness:1},
    {op:'matrix',at:[0,0],rows,legend}];
  step('draw_batch',{frame,layer:19,ops:wing(angles[i]-19,[36,34+by],.83,i>=7?.75:.9,true)});
  step('draw_batch',{frame,layer:2,ops:body});
  step('draw_batch',{frame,layer:20,ops:wing(angles[i],[38,36+by],1,i>=7?.67:1,false)});
}
step('animation_set_timing',{animation:'Voo',durations:[90,70,60,60,70,90,80,70,60,60,70,80],loop_mode:'loop'});
step('transaction_commit',{});
step('animation_preview',{animation:'Voo',direction:'W',scale:3});
step('frame_preview',{frame:6,scale:6});
step('frame_preview',{frame:11,scale:6});
fs.writeFileSync(path.join(dir,'draw-flight.json'),JSON.stringify(steps));
const run=cp.spawnSync(path.resolve('target/debug/psm.exe'),['mcp-run',path.join(dir,'draw-flight.json'),'--images',dir],{encoding:'utf8'});
process.stdout.write(run.stdout);process.stderr.write(run.stderr);process.exit(run.status??1);
