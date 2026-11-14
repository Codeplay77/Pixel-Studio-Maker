/** Aren: authored articulated pixel poses, expanded to SDC and drawn by the Rust MCP server.
 * All final pixels come from draw_batch; no image imports or generative services.
 */
import fs from 'node:fs';
const OUT='examples/hero-aren';
const DIRS=['N','NE','E','SE','S','SW','W','NW'];
const M={
 steel:['#202b39','#344456','#506479','#71899a','#9eafb5','#d8dfcf'],
 cloth:['#102a2b','#193e3c','#295952','#417767','#6b9b7e'],
 leather:['#211c22','#3b2c2b','#594039','#795b45','#a0805a'],
 gold:['#463629','#735438','#a17c48','#c6a46b','#ead3a0'],
 skin:['#593c3a','#825448','#ad7760','#d0a17d','#ebc6a0'],
 hair:['#1b2028','#302a2b','#4a3630','#694b38','#8a6847'],
 dark:['#17202b','#222c39','#344352','#4e5e6b'],
 gem:['#183b3d','#2e7472','#5da69b','#a2d8b3'],
};
const PAL=[...new Set(Object.values(M).flat()),'#141c27','#17212d55','#eff5db','#e2be75'];
const add=(a,b)=>a.map((v,i)=>v+b[i]), sub=(a,b)=>a.map((v,i)=>v-b[i]), mul=(a,s)=>a.map(v=>v*s);
const dot=(a,b)=>a.reduce((s,v,i)=>s+v*b[i],0), cross=(a,b)=>[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
const unit=a=>mul(a,1/(Math.hypot(...a)||1)), mix=(a,b,t)=>add(mul(a,1-t),mul(b,t));
const LIGHT=unit([-.6,.5,1]);
const ANG={S:0,SE:Math.PI/4,E:Math.PI/2,NE:3*Math.PI/4,N:Math.PI,NW:5*Math.PI/4,W:3*Math.PI/2,SW:7*Math.PI/4};

function pose(anim,i,n){
 const t=i/n*Math.PI*2;
 const p={bob:0,lean:0,twist:0,cape:Math.sin(t)*1.6,head:0,blink:false,
 hips:[[-6,0,44],[6,0,44]],knees:[[-7,1,24],[7,1,24]],feet:[[-8,2,3],[8,2,3]],
 elbows:[[-17,0,61],[17,2,61]],hands:[[-17,4,49],[18,7,49]],blade:[.22,.4,.88]};
 if(anim==='idle') {p.bob=[0,.4,1,1.1,.65,.1][i];p.cape=[0,1.2,2,.8,-1,-1.8][i];p.blink=i===4;p.hands[1][1]+=[0,.3,.6,.8,.4,.1][i];}
 if(anim==='walk') {
  p.bob=-1.3*Math.cos(2*t); p.lean=2;
  for(let k=0;k<2;k++) {
   const q=t+k*Math.PI, stride=Math.cos(q)*11, lift=Math.max(0,Math.sin(q))*7;
   p.feet[k]=[(k?1:-1)*7,stride,3+lift];
   p.knees[k]=[(k?1:-1)*6, stride*.5+Math.max(0,Math.sin(q))*5,24+lift*.35];
   p.elbows[k]=[(k?1:-1)*17,-stride*.4,61+p.bob];
   p.hands[k]=[(k?1:-1)*18,4-stride*.65,49+p.bob];
  }
  p.cape=Math.sin(t-1)*3.5;p.blade=[.18,.3,.94];
 }
 if(anim==='attack') {
  const R=[ [19,7,50],[20,-6,68],[16,-13,83],[8,2,78],[10,24,70],[-10,26,50],[-9,16,47],[16,8,49] ];
  const E=[ [17,2,61],[23,-5,72],[22,-7,81],[20,3,75],[21,13,72],[4,16,62],[-3,11,58],[16,4,62] ];
  const V=[ [.2,.4,.9],[.3,-.6,.8],[.3,-.85,.5],[-.2,.5,.84],[-.4,.85,.25],[-.8,.6,-.2],[-.6,.5,.55],[.15,.4,.9] ];
  p.hands[1]=R[i];p.elbows[1]=E[i];p.blade=V[i];
  p.lean=[0,-2,-3,0,5,7,3,0][i];p.twist=[0,-.1,-.18,0,.25,.3,.14,0][i];p.bob=[0,-1,-2,0,-2,-3,-1,0][i];
  p.feet[1]=[9,[2,-3,-4,0,14,15,10,3][i],3];p.knees[1]=[8, p.feet[1][1]*.55+2,23];
  p.hands[0]=[-14,10,57];p.elbows[0]=[-18,3,65];p.cape=[0,1,3,6,8,6,3,1][i];
 }
 if(anim==='hurt') {
  p.lean=[-2,-7,-5,-1][i];p.bob=[-1,-4,-3,-1][i];p.head=[-1,-3,-2,0][i];p.cape=[2,5,3,0][i];
  p.hands[0]=[-10,10,64+p.bob];p.elbows[0]=[-17,4,62+p.bob];p.hands[1]=[20,-3,49+p.bob];p.elbows[1]=[21,-4,61+p.bob];p.blade=[.4,-.3,.85];
  p.feet[0]=[-9,-5,3];p.knees[0]=[-8,-2,22];
 }
 return p;
}

function artwork(direction,anim,index,n) {
 const p=pose(anim,index,n), angle=ANG[direction], co=Math.cos(angle),si=Math.sin(angle);let surfaces=[];
 const rotate=v=>[v[0]*co+v[1]*si,v[1]*co-v[0]*si,v[2]];
 const project=v=>{const w=rotate(v);return [Math.round(64+w[0]),Math.round(111-w[2]*.96+w[1]*.29)];};
 const depth=v=>{const w=rotate(v);return w[1]*.96+w[2]*.29;};
 const upper=v=> {const z=v[2],tw=p.twist;return [v[0]*Math.cos(tw)+v[1]*Math.sin(tw), v[1]*Math.cos(tw)-v[0]*Math.sin(tw)+p.lean*Math.max(0,(z-44)/40),z+p.bob];};
 function face(v,mat,center,bias=0,fixed){
  let normal=unit(cross(sub(v[1],v[0]),sub(v[2],v[0])));
  const c=mul(v.reduce((a,x)=>add(a,x),[0,0,0]),1/v.length);
  if(center && dot(normal,sub(c,center))<0) normal=mul(normal,-1);
  const w=rotate(normal),ramp=M[mat];
  let light=.22+Math.max(0,dot(w,LIGHT))*.7+bias;
  if(mat==='steel' && dot(w,unit(add(LIGHT,[0,.96,.29])))>.91) light+=.2;
  const color=fixed??ramp[Math.max(0,Math.min(ramp.length-1,Math.round(light*(ramp.length-1))))];
  const points=v.map(project);if(new Set(points.map(x=>x.join(','))).size<3)return;
  surfaces.push({d:depth(c),op:{op:'polygon',points,color,fill:true}});
 }
 function ellipsoid(c,r,mat,segments=10,bands=6,transform=false){
  const tx=v=>transform?upper(v):v,C=tx(c);
  for(let j=0;j<bands;j++)for(let k=0;k<segments;k++) {
   const vertex=(a,b)=>[c[0]+r[0]*Math.sin(a)*Math.cos(b),c[1]+r[1]*Math.sin(a)*Math.sin(b),c[2]+r[2]*Math.cos(a)];
   const a=j/bands*Math.PI,b=k/segments*Math.PI*2,aa=(j+1)/bands*Math.PI,bb=(k+1)/segments*Math.PI*2;
   face([vertex(a,b),vertex(aa,b),vertex(aa,bb),vertex(a,bb)].map(tx),mat,C);
  }
 }
 function tube(a,b,ra,rb,mat,segments=8,transform=false){
  if(transform){a=upper(a);b=upper(b);}
  const axis=unit(sub(b,a)),u=unit(cross(axis,Math.abs(axis[2])>.9?[0,1,0]:[0,0,1])),v=cross(axis,u);
  const ring=(c,r,k)=>add(c,add(mul(u,Math.cos(k/segments*Math.PI*2)*r),mul(v,Math.sin(k/segments*Math.PI*2)*r)));
  for(let k=0;k<segments;k++)face([ring(a,ra,k),ring(b,rb,k),ring(b,rb,k+1),ring(a,ra,k+1)],mat,mix(a,b,.5));
  face(Array.from({length:segments},(_,k)=>ring(b,rb,k)),mat,sub(b,axis));
 }
 function torso(rings,mat) {
  const N=12;
  for(let j=0;j<rings.length-1;j++)for(let k=0;k<N;k++) {
   const ring=(r,k)=>upper([Math.cos(k/N*Math.PI*2)*r[1],Math.sin(k/N*Math.PI*2)*r[2],r[0]]);
   face([ring(rings[j],k),ring(rings[j+1],k),ring(rings[j+1],k+1),ring(rings[j],k+1)],mat,upper([0,0,(rings[j][0]+rings[j+1][0])/2]));
  }
 }
 // Cape: eight folded panels, tapered shoulder attachment and a wind-shaped hem.
 for(let row=0;row<4;row++)for(let col=0;col<8;col++) {
  const v=(r,c)=>{const f=r/4,side=(c/8*2-1);return upper([side*(11+f*4),-5-f*(5+p.cape*.35)+Math.cos(c*Math.PI)*1.1*f,78-f*49+Math.sin(c*1.4+index)*f*1.1]);};
  face([v(row,col),v(row+1,col),v(row+1,col+1),v(row,col+1)],'cloth',upper([0,5,55]), col%2 ? -.03:.23);
 }
 // Embroidered back sigil follows the cape plane, without changing handedness.
 const capeY=z=>-5-(78-z)/49*(5+p.cape*.35)-1.5;
 const emblem=[[0,69],[-3,64],[0,59],[3,64],[0,69]];
 for(let j=0;j<emblem.length-1;j++) {
  const [x,z]=emblem[j],[xx,zz]=emblem[j+1];
  tube([x,capeY(z),z],[xx,capeY(zz),zz],.45,.45,'gold',5,true);
 }
 // Boots, articulated legs, knee plates and greaves.
 for(let k=0;k<2;k++) {
  const hip=add(p.hips[k],[0,0,p.bob]),knee=p.knees[k],ankle=p.feet[k];
  tube(hip,knee,5.3,3.8,'dark');tube(mix(hip,knee,.08),mix(hip,knee,.78),5.5,4.1,'steel');
  ellipsoid(knee,[4.6,4.2,4.3],'steel',8,4);
  tube(knee,ankle,3.5,2.5,'leather');tube(mix(knee,ankle,.17),mix(knee,ankle,.82),3.7,2.8,'steel');
  tube(mix(knee,ankle,.7),mix(knee,ankle,.76),3.7,3.6,'leather');
  ellipsoid(add(ankle,[0,2.4,-.6]),[3.6,6,2.5],'leather',8,4);
  tube(add(ankle,[-2.7,6,-1]),add(ankle,[2.7,6,-1]),.7,.7,'dark',6);
 }
 // Torso: mail, flared hip plates, fitted breastplate, belt and collar.
 torso([[42,9,5],[49,9,5],[65,12,6],[76,12,5]],'dark');
 torso([[39,11,6],[45,10,5.8],[51,8.4,5.1]],'steel');
 for(let z of [41,45,49])torso([[z,10.3-(z-41)*.2,5.8],[z+1,10.3-(z-41)*.2,5.8]],'dark');
 torso([[51,8.4,5],[59,9,5],[69,12,6],[75,11,5]],'steel');
 torso([[50,9.6,5.8],[53,9.6,5.8]],'leather');
 face([[-2,6,50],[-2,6,53],[2,6,53],[2,6,50]].map(upper),'gold',upper([0,0,51]));
 tube([0,0,75],[0,0,82],3.7,3.2,'dark',10,true);
 torso([[75,7,4],[77,6,4]],'gold');
 // Teal cloth tabard and stitched hem.
 face([[-4,6.1,51],[4,6.1,51],[5,6.8,36],[-4,6.8,34]].map(upper),'cloth',upper([0,0,43]),.12);
 tube([-3,6.9,35],[4,6.9,37],.55,.55,'gold',5,true);
 // Sun-shaped crest on the cuirass, rivets and a diagonal leather baldric.
 face([[0,6.3,72],[3,6.6,68],[0,6.9,64],[-3,6.6,68]].map(upper),'gold',upper([0,0,68]),.25);
 ellipsoid([0,6.9,68],[1.1,.7,1.6],'gem',6,4,true);
 tube([-9,4.4,75],[7,5.3,53],1.1,1.1,'leather',6,true);
 for(const x of [-8,8])for(const z of [57,71])ellipsoid([x,5.1,z],[.65,.5,.65],'gold',6,3,true);
 // Arms retain physical handedness in all eight directions.
 for(let k=0;k<2;k++) {
  const side=k?1:-1,shoulder=[side*13,0,74],elbow=p.elbows[k],hand=p.hands[k];
  tube(shoulder,elbow,4,3.1,'dark',8,true);
  tube(mix(shoulder,elbow,.2),mix(shoulder,elbow,.78),4.1,3.3,'steel',8,true);
  ellipsoid(elbow,[3.5,3.8,3.4],'steel',8,4,true);
  tube(elbow,hand,3.3,2.1,'leather',8,true);
  tube(mix(elbow,hand,.3),mix(elbow,hand,.86),3.5,2.5,'steel',8,true);
  ellipsoid(shoulder,[6,5.8,4.5],'steel',10,5,true);
  tube(add(shoulder,[-4,2,-2]),add(shoulder,[4,2,-2]),.75,.75,'gold',6,true);
  ellipsoid(hand,[2.6,2.3,3.1],'leather',8,4,true);
 }
 // Head with readable face planes, ears, brown hair, nose, eyes and short beard.
 const headZ=89+p.head;
 ellipsoid([0,0,headZ],[5.7,4.7,7.5],'skin',12,8,true);
 ellipsoid([0,-1.3,headZ+4.8],[6.2,5.1,4.4],'hair',12,6,true);
 ellipsoid([0,-3.4,headZ-.5],[5.5,2.6,6.7],'hair',10,6,true);
 for(const x of [-5.7,5.7])ellipsoid([x,0,headZ],[1.2,1.5,2.1],'skin',6,4,true);
 ellipsoid([0,4.6,headZ-.7],[1.2,1.8,1.8],'skin',8,4,true);
 ellipsoid([0,3.2,headZ-4.8],[3.8,2.3,2.2],'hair',10,4,true);
 for(const x of [-2.3,2.3]) {
  tube([x-1,4.3,headZ+2],[x+1,4.3,headZ+2.2],.45,.35,'hair',5,true);
  ellipsoid([x,4.55,headZ+1],[1,.5,p.blink?.18:.48],'dark',6,4,true);
  if(!p.blink)ellipsoid([x-.3,4.9,headZ+1.2],[.3,.2,.25],'gold',4,3,true);
 }
 // Sword: diamond-section blade; separate darker face and bright cutting edge.
 const hand=upper(p.hands[1]),axis=unit(p.blade),tip=add(hand,mul(axis,34));
 const crossAxis=unit(cross(axis,[0,1,0])),flat=unit(cross(axis,crossAxis));
 const guard=add(hand,mul(axis,4.3)),base=add(hand,mul(axis,6));
 tube(add(hand,mul(axis,-3.5)),guard,1,1,'leather',7);
 tube(add(guard,mul(crossAxis,-5)),add(guard,mul(crossAxis,5)),.9,.6,'gold',6);
 ellipsoid(add(hand,mul(axis,-4)),[1.5,1.5,1.5],'gold',6,4);
 const left=add(base,mul(crossAxis,2)),right=add(base,mul(crossAxis,-2)),ridge=add(base,mul(flat,1));
 face([left,tip,ridge],'steel',sub(base,flat),.15);face([ridge,tip,right],'steel',sub(base,flat),-.1);
 tube(add(base,mul(flat,1.15)),tip,.32,.15,'steel',5);
 // Dark scabbard and brass chape on the opposite hip.
 tube(upper([-9,-1,51]),upper([-14,-5,24]),1.5,1.1,'leather',7);
 tube(upper([-13.4,-4.6,27]),upper([-14,-5,24]),1.6,1.2,'gold',6);
 surfaces.sort((a,b)=>a.d-b.d);
 const ops=surfaces.map(s=>s.op);
 ops.push({op:'outline',color:'#141c27',mode:'outside'});
 return {ops, project, pose:p};
}
const configs={idle:{n:6,ms:180,loop:'loop'},walk:{n:8,ms:95,loop:'loop'},attack:{n:8,ms:90,loop:'once'},hurt:{n:4,ms:110,loop:'once'}};
const mode=process.argv[2]??'preview';
const project=process.argv[3];
const steps=[];
const push=(tool,args={},extra={})=>steps.push({tool,args:{...(project?{project}:{}),...args},...extra});
if(mode==='preview'){
 push('project_create',{name:'Aren - Guardião de Esmeralda',width:128,height:128,sprite_name:'Aren',palette:PAL,palette_name:'Aren - aço, couro e esmeralda'});
 push('layer_properties',{layer:'Layer 1',name:'Personagem'});
 push('sprite_update',{origin:[64,111]});
 push('draw_batch',{layer:'Personagem',label:'Aren - pose-base S',ops:artwork('S','idle',0,6).ops});
 push('frame_preview',{scale:5},{save_as:'aren-base'});
 push('project_save',{path:`${OUT}/aren.psmproj`});
} else if(mode==='samples') {
 for(const direction of DIRS) {
  push('draw_batch',{layer:'Personagem',frame:3,label:`Estudo ${direction}`,ops:[{op:'clear'},...artwork(direction,'idle',0,6).ops]});
  push('frame_preview',{frame:3,scale:4},{save_as:`view-${direction}`});
 }
 for(const index of [2,3,4,5]) {
  push('draw_batch',{layer:'Personagem',frame:3,label:`Estudo ataque ${index}`,ops:[{op:'clear'},...artwork('SE','attack',index,8).ops]});
  push('frame_preview',{frame:3,scale:4},{save_as:`attack-${index}`});
 }
 push('draw_batch',{layer:'Personagem',frame:3,label:'Aren - pose-base',ops:[{op:'clear'},...artwork('S','idle',0,6).ops]});
 push('project_save',{path:`${OUT}/aren.psmproj`});
} else if(mode==='animate') {
 for(const [anim,c] of Object.entries(configs)) {
  push('animation_create',{name:anim,directions:DIRS,frames:c.n,duration_ms:c.ms,loop_mode:c.loop,tags:['Aren','8-direcoes']});
  push('animation_update',{animation:anim,cycle:{notes:{idle:'Respiração, piscar e tecido ao vento.',walk:'Oito poses com contato, passagem e balanço oposto dos braços.',attack:'Guarda, antecipação, espada alta, corte e recuperação.',hurt:'Recuo do tronco, proteção e recuperação.'}[anim]}});
  for(const direction of DIRS)for(let i=0;i<c.n;i++) {
   const frame={animation:anim,direction,index:i};
   push('draw_batch',{layer:'Personagem',frame,label:`${anim} ${direction} ${i+1}/${c.n}`,ops:artwork(direction,anim,i,c.n).ops});
  }
  push('project_save',{path:`${OUT}/aren.psmproj`});
  push('animation_preview',{animation:anim,direction:'SE',scale:2},{save_as:`${anim}-SE-strip`});
 }
} else if(mode==='export') {
 for(const [anim,c] of Object.entries(configs)) {
  push('export_spritesheet',{path:`${OUT}/exports/aren-${anim}.png`,selection:{animations:[anim]},sheet:{layout:'rows',spacing:0}});
  for(const direction of DIRS)push('export_gif',{path:`${OUT}/exports/gifs/${anim}-${direction}.gif`,animation:anim,direction,scale:3});
 }
 push('export_spritesheet',{path:`${OUT}/exports/aren-completo.png`,selection:{animations:Object.keys(configs)},sheet:{layout:'rows',spacing:0}});
 push('project_validate',{});push('project_save',{path:`${OUT}/aren.psmproj`});push('project_info',{});
}
fs.mkdirSync(OUT,{recursive:true});
fs.writeFileSync(`${OUT}/${mode}.json`,JSON.stringify(steps));
console.log(`${steps.length} MCP commands -> ${OUT}/${mode}.json`);
