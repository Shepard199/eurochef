import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class CurrentAttackerBytePatternEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address cur=currentProgram.getMinAddress();
  int count=0;
  while(cur!=null){
   Address found=findBytes(cur,"10 2A 7B 00");
   if(found==null) break;
   Function f=getFunctionContaining(found);
   println("FOUND "+found+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
   count++;
   if(found.equals(currentProgram.getMaxAddress())) break;
   cur=found.add(1);
  }
  println("COUNT="+count);
 }
}