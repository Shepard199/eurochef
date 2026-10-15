import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.symbol.*;
public class InputUpDownStringEvidence extends GhidraScript {
 public void run()throws Exception{
  Memory m=currentProgram.getMemory();
  String[] ss={"(UP)","(DOWN)","UP","DOWN","MenuUp","MenuDown","Select","Back"};
  for(String s:ss){
   byte[] b=(s+"\0").getBytes(java.nio.charset.StandardCharsets.US_ASCII);
   Address cur=currentProgram.getMinAddress();
   println("\nSEARCH "+s);
   int hits=0;
   while(cur!=null && hits<40){
    Address a=m.findBytes(cur,b,null,true,monitor);
    if(a==null)break;
    println("HIT "+a);
    ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(a);
    while(it.hasNext()){
      Reference r=it.next();
      println("  REF "+r.getFromAddress()+" "+r.getReferenceType());
    }
    cur=a.add(1); hits++;
   }
  }
 }
}