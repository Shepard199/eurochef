import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
public class GlobalThrottleManagerEvidence extends GhidraScript {
 private void dc(Function f,DecompInterface d)throws Exception{
  if(f==null)return;
  println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");
  DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  Address g=toAddr(0x007B29E0L);
  println("REFS TO DAT_007B29E0");
  for(Reference ref:getReferencesTo(g)){
    Function f=getFunctionContaining(ref.getFromAddress());
    println(ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
    dc(f,d);
  }
  d.dispose();
 }
}