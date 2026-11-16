use std::fmt::Write;
use vectorio_core::generate::GenOutput;
#[allow(dead_code)]
pub fn paths(svg:&mut String,output:&GenOutput){
    for part in &output.parts{
        let color=match part.name.as_str(){"vegetación"=>"#627754","sombra"=>"#9E8773","terreno"=>"#80684D","gente"=>"#B0413E","rótulos"=>"#35216C","agua"=>"#3A6EA5","tejados"=>"#B4583A",_=>"#253238"};
        write!(svg,r#"<g data-name="{}" fill="none" stroke="{color}" stroke-width="0.25" stroke-linecap="round" stroke-linejoin="round">"#,part.name).unwrap();
        for line in &part.lines{svg.push_str("<polyline points=\"");for q in line{write!(svg,"{:.5},{:.5} ",q.x,q.y).unwrap();}svg.push_str("\"/>");}
        svg.push_str("</g>");
    }
}

#[allow(dead_code)]
pub fn colored_paths(svg:&mut String,output:&GenOutput,plan:&vectorio_core::generate::InkPlan,choice:&vectorio_core::generate::InkChoice){
    for (i,part) in output.parts.iter().enumerate(){
        let ink=plan.ink_for(choice,i,&part.name);let [r,g,b]=ink.color;
        write!(svg,r##"<g data-name="{}" fill="none" stroke="#{r:02x}{g:02x}{b:02x}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round">"##,part.name,ink.width_mm).unwrap();
        for line in &part.lines{svg.push_str("<polyline points=\"");for q in line{write!(svg,"{:.5},{:.5} ",q.x,q.y).unwrap();}svg.push_str("\"/>");}
        svg.push_str("</g>");
    }
}
