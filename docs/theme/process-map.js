// Interactive companion to the How Esdieff works chapter.
(() => {
  const root = document.getElementById('esdieff-process-map');
  if (!root) return;
  function append(tag, id) {
    const element = document.createElement(tag);
    element.id = id;
    root.append(element);
    return element;
  }
  function selector(id, text) {
    const container = append('p', id + '-control');
    const label = document.createElement('label');
    label.htmlFor = id;
    label.textContent = text;
    const select = document.createElement('select');
    select.id = id;
    select.className = 'select select-sm';
    container.append(label, document.createElement('br'), select);
    return select;
  }
  const workflow = selector('esdieff-workflow', 'Workflow');
  const flows = [['overview', 'Overview'], ['check', 'Check and inspect'],
    ['compare', 'Compare revisions'], ['values', 'Test values'], ['create', 'Author and edit']];
  for (const [value, text] of flows) {
    const option = document.createElement('option');
    option.value = value;
    option.textContent = text;
    workflow.append(option);
  }
  const examples = selector('esdieff-example', 'Example');
  const exampleControl = examples.parentElement;
  const host = append('div', 'esdieff-diagram');
  const selected = append('p', 'esdieff-selected');
  selected.setAttribute('role', 'status');
  selected.setAttribute('aria-live', 'polite');
  const commands = append('p', 'esdieff-command');
  const NS = 'http://www.w3.org/2000/svg';
  const cases = {
    check: [['complete', 'Complete inputs'], ['missing', 'Missing dependency'], ['invalid', 'Invalid definition']],
    compare: [['maximum', 'Raise readable maximum'], ['description', 'Change description'], ['step', 'Remove step constraint']],
    values: [['good', 'Admitted value'], ['bad', 'Rejected value'], ['expected', 'Expected rejection']]
  };
  const defaults = {check:'complete', compare:'maximum', values:'good'};
  let flow = 'overview';
  let choice = {...defaults};
  let lastWidth = 0;
  const node = (id, title, subtitle, tone = 'neutral') => ({id,title,subtitle,tone});
  const edge = (from, to) => ({from,to});
  function data() {
    if (flow === 'overview') return {
      title:'From supplied documents to useful evidence',
      summary:'Local SDF files form a catalogue. Source inspection reads retained content. Effective views resolve supplied definitions and report implicit defaults separately. Model checking, revision comparison and value checking use the resulting meaning and produce located evidence.',
      rows:[
        [node('local','Local SDF files','Models + shared definitions')],
        [node('catalogue','Catalogue','Explicit inputs · retained content · exact numbers · source locations')],
        [node('source','Source inspection','Inspect retained source JSON'),node('effective','Effective view','Compose references + refinements · report defaults separately · retain origins')],
        [node('check','Check models','Assess the requested model rules'),node('compare','Compare revisions','Earlier + later effective interfaces'),node('values','Test values','Check samples · generate · check traces')],
        [node('report','Results with evidence','Outcome · scope · coverage where applicable · findings + locations')]
      ],
      edges:[edge('local','catalogue'),edge('catalogue','source'),edge('catalogue','effective'),edge('effective','check'),edge('effective','compare'),edge('effective','values'),edge('source','report'),edge('check','report'),edge('compare','report'),edge('values','report')],
      detail:'Model operations read supplied local files and resolve references only within those files. They run offline.',
      command:'CLI and Rust library share these model operations. Author and edit produce new SDF documents that can enter the same flow.'
    };
    if (flow === 'check') {
      const c = choice.check;
      return {
        title:'Model checking and inspection',
        summary:'The selected scope and assessment determine the requested work. Source structure and effective model meaning are assessed. Errors fail the check, otherwise unfinished work is undetermined, otherwise it passes. Completeness is reported separately. Inspection does not require a passed model check.',
        rows:[
          [node('scope','Choose scope','All supplied models, or selected roots + dependencies'),node('profile','Choose assessment','Initial · expanded profiles · full baseline')],
          [node('structure','Assess source structure',c==='invalid'?'Example: "type": 7 → data.string error':'JSON · names · definition roles · namespaces',c==='invalid'?'negative':'neutral')],
          [node('effective','Establish effective meaning',c==='missing'?'Needed shared file absent → reference unavailable':'Resolve references · merge refinements · retain source origins',c==='missing'?'unknown':'neutral')],
          [node('meaning','Assess established content','Parameters · relationships · supported model rules')],
          [node('passed','Passed','No failing finding + requested work complete',c==='complete'?'positive':'neutral'),node('failed','Failed','Error, or warning in strict mode',c==='invalid'?'negative':'neutral'),node('unknown','Undetermined','Unfinished work, with no failing finding',c==='missing'?'unknown':'neutral')],
          [node('report','Report outcome and completeness separately','Requested coverage + broader coverage · conditions · source/effective locations')]
        ],
        edges:[edge('scope','structure'),edge('profile','structure'),edge('structure','effective'),edge('effective','meaning'),edge('meaning','passed'),edge('meaning','failed'),edge('meaning','unknown'),edge('passed','report'),edge('failed','report'),edge('unknown','report')],
        detail:c==='complete'?'Passed applies to the selected assessment. It does not establish every RFC obligation or device behaviour.':c==='missing'?'Missing dependency: requested meaning remains undetermined. Independent findings can still be reported.':'Invalid definition: a definite error fails the check, even if other requested work remains unfinished.',
        command:'inspect --view source reads original content independently. inspect --view effective shows the composed definition and its origins.'
      };
    }
    if (flow === 'compare') {
      const c = choice.compare;
      const result = c==='maximum'?'negative':c==='description'?'positive':'unknown';
      return {
        title:'Compare from the existing client’s point of view',
        summary:'Two supplied revisions are matched by source keys and corresponding definitions. Effective differences are classified from the old client perspective. Breaking differences have a rule or witness, compatible differences have a proof, and unsupported reasoning stays undetermined. Acknowledgements retain the classifications while changing the exit decision.',
        rows:[
          [node('old','Earlier revision',c==='description'?'Original description':'Readable + writable · minimum 5 · maximum 30 · step 0.5'),node('new','Later revision',c==='maximum'?'Readable + writable · minimum 5 · maximum 35 · step 0.5':c==='description'?'Updated description; same value constraints':'Readable + writable · minimum 5 · maximum 30 · step removed')],
          [node('oldview','Earlier effective interface','Resolve its supplied references'),node('newview','Later effective interface','Resolve its supplied references')],
          [node('align','Match and compare effective content','Source keys + corresponding definitions · inherited changes included')],
          [node('direction','Assess effects on old clients','Client-supplied data: writes / action inputs · thing-supplied data: reads / outputs')],
          [node('breaking','Breaking','A rule or witness proves a break',result==='negative'?'negative':'neutral'),node('compatible','Compatible','Compatibility established',result==='positive'?'positive':'neutral'),node('undetermined','Undetermined','Neither break nor compatibility proved',result==='unknown'?'unknown':'neutral')],
          [node('ack','Apply optional acknowledgements','Keep each classification · only unacknowledged differences decide disposition')]
        ],
        edges:[edge('old','oldview'),edge('new','newview'),edge('oldview','align'),edge('newview','align'),edge('align','direction'),edge('direction','breaking'),edge('direction','compatible'),edge('direction','undetermined'),edge('breaking','ack'),edge('compatible','ack'),edge('undetermined','ack')],
        detail:c==='maximum'?'Witness: a device may now report 35, which an old client expecting at most 30 cannot accept.':c==='description'?'Description-only change: the effective value constraints remain the same, so the difference is compatible.':'Step removed: current comparison reasoning leaves this difference undetermined. It is not reported as safe.',
        command:'esdieff diff --old v1/ --new v2/ · Results include differences, directions, origins and any breaking witnesses.'
      };
    }
    if (flow === 'values') {
      const c = choice.values;
      const good = c==='good';
      return {
        title:'One value-checking engine, three workflows',
        summary:'A selected effective data description supplies the constraints. Single samples, generation candidates and individual trace entries use Sample-value checking. Admissions are admitted, rejected or undetermined. Generation records settled results. Trace entries compare admission with an expected admission, defaulting to admitted.',
        rows:[
          [node('model','Selected effective description','Example: target temperature · 5–30 · step 0.5 · non-null')],
          [node('single','sample','One supplied JSON value'),node('generate','generate','Propose representative, boundary + invalid candidates'),node('trace','trace','Recorded or generated JSONL · each entry checked independently')],
          [node('engine','Sample-value checking','Check the value against established constraints · exact numeric rules')],
          [node('admitted','Admitted','Value fits established constraints',good?'positive':'neutral'),node('rejected','Rejected','An established constraint excludes it',!good?'negative':'neutral'),node('unknown','Undetermined','Needed meaning or capability unavailable')],
          [node('sampleout','Sample result','Admission + reasons'),node('genout','Generated vectors','Values + settled expectations · reusable JSONL'),node('traceout','Trace result','Compare admission with expected · per-entry pass / fail / undetermined')]
        ],
        edges:[edge('model','single'),edge('model','generate'),edge('model','trace'),edge('single','engine'),edge('generate','engine'),edge('trace','engine'),edge('engine','admitted'),edge('engine','rejected'),edge('engine','unknown'),edge('admitted','sampleout'),edge('rejected','genout'),edge('unknown','traceout')],
        sharedOutputBus:true,
        detail:good?'21.5 → admitted. A trace entry with no explicit expectation passes because admission is the default expectation.':c==='bad'?'21.3 → rejected: it is not a multiple of 0.5. A trace entry expecting admission fails.':'31 → rejected; expected: rejected → trace entry passed. A rejected value can be a successful test.',
        command:'Generated JSONL can be checked again with trace against another model revision. Trace checks values independently, with no timing or protocol semantics.'
      };
    }
    return {
      title:'Build a candidate, establish fidelity, then deliver',
      summary:'Author composes a new sdfThing using references and refinements. Edit replaces or removes existing authored qualities. Both produce a candidate without changing source inputs. Delivery requires complete generation, established fidelity and a passed operation outcome, including any requested model check.',
      rows:[
        [node('recipe','author plan + supplied objects','Names · references · refinements · required objects'),node('edits','edit plan + existing source','Explicit replacements or removals of present qualities')],
        [node('author','Compose a new sdfThing','Preserve links to supplied definitions'),node('edit','Apply authored-quality edits','Preserve other content and exact number spellings')],
        [node('candidate','Generate candidate SDF','Original inputs remain unchanged')],
        [node('fidelity','Establish fidelity','Confirm intended references / refinements or edits + preservation')],
        [node('checking','Run model check if requested','Use the normal assessment engine on the generated result')],
        [node('gate','Document deliverable?','Generation complete + fidelity established + operation passed')],
        [node('write','Write new SDF document','Ready for inspect, check or diff','positive'),node('withhold','Withhold document output','Report failed or undetermined operation','unknown')]
      ],
      edges:[edge('recipe','author'),edge('edits','edit'),edge('author','candidate'),edge('edit','candidate'),edge('candidate','fidelity'),edge('fidelity','checking'),edge('checking','gate'),edge('gate','write'),edge('gate','withhold')],
      detail:'Fidelity and model validity answer different questions. The candidate and its report remain inspectable even when document delivery is withheld.',
      command:'esdieff author --plan recipe.json … --check initial-v1 · esdieff edit --plan edits.json … --check initial-v1'
    };
  }
  function svgElement(tag, attrs = {}, text = '') {
    const el = document.createElementNS(NS, tag);
    for (const [key,value] of Object.entries(attrs)) el.setAttribute(key,String(value));
    if (text) el.textContent = text;
    return el;
  }
  const measure = document.createElement('canvas').getContext('2d');
  function wrap(text,width,font) {
    measure.font=font;
    const result=[];
    let line='';
    for (const word of text.split(' ')) {
      const attempt=line?line+' '+word:word;
      if(line && measure.measureText(attempt).width>width){result.push(line);line=word;}
      else line=attempt;
    }
    if(line)result.push(line);
    return result;
  }
  function draw() {
    const d=data();
    const width=Math.max(180, Math.round(host.getBoundingClientRect().width));
    lastWidth=width;
    const narrow=width<560;
    const computed=getComputedStyle(root);
    const base=parseFloat(computed.fontSize)||16;
    const small=base*0.875;
    const family=computed.fontFamily;
    const line=base*1.5;
    const smallLine=small*1.5;
    const pad=narrow?18:12;
    const gap=narrow?22:18;
    const rowGap=38;
    const coords=new Map();
    const ordered=[];
    let y=16;
    for(let rowIndex=0;rowIndex<d.rows.length;rowIndex++) {
      const row=d.rows[rowIndex];
      const groups=narrow?row.map(n=>[n]):[row];
      for (const group of groups) {
        const boxW=(width-pad*2-gap*(group.length-1))/group.length;
        let rowHeight=0;
        const groupItems=group.map((n,index)=>{
          const title=wrap(n.title,boxW-26,`500 ${base}px ${family}`);
          const sub=wrap(n.subtitle,boxW-26,`400 ${small}px ${family}`);
          const h=24+title.length*line+sub.length*smallLine+5;
          rowHeight=Math.max(rowHeight,h);
          return {...n,x:pad+index*(boxW+gap),y,w:boxW,titleLines:title,subLines:sub,row:rowIndex,order:ordered.length+index};
        });
        for(const n of groupItems){n.h=rowHeight;coords.set(n.id,n);ordered.push(n);}
        y+=rowHeight+(narrow?gap:rowGap);
      }
      if(narrow)y+=16;
    }
    const svg=svgElement('svg',{width:'100%',height:y-(narrow?gap+16:rowGap)+16,viewBox:`0 0 ${width} ${y-(narrow?gap+16:rowGap)+16}`,role:'img','aria-labelledby':'esdieff-svg-title esdieff-svg-description'});
    svg.append(svgElement('title',{id:'esdieff-svg-title'},d.title),svgElement('desc',{id:'esdieff-svg-description'},d.summary));
    const defs=svgElement('defs');
    const marker=svgElement('marker',{id:'esdieff-arrow',viewBox:'0 0 8 8',refX:7,refY:4,markerWidth:7,markerHeight:7,orient:'auto-start-reverse'});
    marker.append(svgElement('path',{d:'M 1 1 L 7 4 L 1 7',fill:'none',stroke:'var(--icons)','stroke-width':1.2}));
    defs.append(marker);svg.append(defs);
    const wires=svgElement('g',{'aria-hidden':'true',fill:'none',stroke:'var(--icons)','stroke-width':1.15,'stroke-opacity':0.7});
    function wire(path){wires.append(svgElement('path',{d:path,'marker-end':'url(#esdieff-arrow)'}));}
    const connections=d.sharedOutputBus?d.edges.filter(e=>!['admitted','rejected','unknown'].includes(e.from)):d.edges;
    for(const e of connections){
      const a=coords.get(e.from), b=coords.get(e.to);
      const ax=a.x+a.w/2, ay=a.y+a.h, bx=b.x+b.w/2;
      if(narrow && b.order>a.order+1){
        const rail=9+(a.order%3)*5;
        wire(`M ${a.x} ${a.y+a.h/2} H ${rail} V ${b.y+b.h/2} H ${b.x-2}`);
      } else if(!narrow && b.row>a.row+1){
        wire(`M ${a.x} ${a.y+a.h/2} H 4 V ${b.y+b.h/2} H ${b.x-2}`);
      } else {
        const middle=(ay+b.y)/2;
        wire(`M ${ax} ${ay} V ${middle} H ${bx} V ${b.y-2}`);
      }
    }
    if(d.sharedOutputBus){
      const from=['admitted','rejected','unknown'].map(id=>coords.get(id));
      const to=['sampleout','genout','traceout'].map(id=>coords.get(id));
      if(!narrow){
        const busY=(from[0].y+from[0].h+to[0].y)/2;
        wires.append(svgElement('path',{d:`M ${from[0].x+from[0].w/2} ${busY} H ${from[2].x+from[2].w/2}`}));
        for(const a of from)wires.append(svgElement('path',{d:`M ${a.x+a.w/2} ${a.y+a.h} V ${busY}`}));
        for(const b of to)wire(`M ${b.x+b.w/2} ${busY} V ${b.y-2}`);
      }else{
        const rail=9;
        for(const a of from)wires.append(svgElement('path',{d:`M ${a.x} ${a.y+a.h/2} H ${rail}`}));
        wires.append(svgElement('path',{d:`M ${rail} ${from[0].y+from[0].h/2} V ${to[2].y+to[2].h/2}`}));
        for(const b of to)wire(`M ${rail} ${b.y+b.h/2} H ${b.x-2}`);
      }
    }
    svg.append(wires);
    const series={positive:'var(--term-green)',negative:'var(--term-red)',unknown:'var(--term-yellow)'};
    for(const n of ordered){
      const g=svgElement('g');
      g.append(svgElement('rect',{x:n.x,y:n.y,width:n.w,height:n.h,rx:8,fill:n.tone==='neutral'?'var(--quote-bg)':`color-mix(in srgb, ${series[n.tone]} 13%, var(--bg))`,stroke:'var(--doc-line)','stroke-width':1}));
      let ty=n.y+15+base;
      for(const t of n.titleLines){g.append(svgElement('text',{x:n.x+n.w/2,y:ty,'text-anchor':'middle','font-weight':500,fill:'var(--fg)'},t));ty+=line;}
      ty+=2-(base-small);
      for(const t of n.subLines){g.append(svgElement('text',{x:n.x+n.w/2,y:ty,'text-anchor':'middle','font-size':small,'font-weight':400,fill:'var(--fg)'},t));ty+=smallLine;}
      svg.append(g);
    }
    host.replaceChildren(svg);
    selected.textContent=d.detail;
    commands.textContent=d.command;
  }
  function save() {
    const url = new URL(window.location.href);
    url.searchParams.set('workflow', flow);
    if (cases[flow]) url.searchParams.set('example', choice[flow]);
    else url.searchParams.delete('example');
    window.history.replaceState(null, '', url);
  }
  function renderExamples() {
    examples.replaceChildren();
    exampleControl.hidden = !cases[flow];
    for (const [id, text] of cases[flow] || []) {
      const option = document.createElement('option');
      option.value = id;
      option.textContent = text;
      option.selected = choice[flow] === id;
      examples.append(option);
    }
  }
  function render() {
    workflow.value = flow;
    renderExamples();
    draw();
  }
  const params = new URLSearchParams(window.location.search);
  const requested = params.get('workflow');
  if (flows.some(([id]) => id === requested)) flow = requested;
  const example = params.get('example');
  if (cases[flow]?.some(c => c[0] === example)) choice[flow] = example;
  workflow.addEventListener('change', () => { flow = workflow.value; render(); save(); });
  examples.addEventListener('change', () => { choice[flow] = examples.value; draw(); save(); });
  render();
  document.getElementById('esdieff-process-static').hidden = true;
  new ResizeObserver(() => {
    const width = Math.round(host.getBoundingClientRect().width);
    if (width !== lastWidth) draw();
  }).observe(host);
})();
