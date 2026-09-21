import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.listing.Function;
public class GlobalAiManagerUpdateCallerEvidence extends GhidraScript {
 @Override public void run()throws Exception{
  Address a=toAddr(0x004561F0L);
  println("REFS TO 004561F0");
  for(Reference r:getReferencesTo(a)){
   Function f=getFunctionContaining(r.getFromAddress());
   println(r.getFromAddress()+" "+r.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
  }
 }
}