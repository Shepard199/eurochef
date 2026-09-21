import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
public class PursueNavNodeProvenanceEvidence extends GhidraScript {
 private void dc(Function f,DecompInterface d)throws Exception{
  if(f==null)return;
  println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  Address a=toAddr(0x0046DFB0L);
  println("REFS TO 0046DFB0");
  for(Reference ref:getReferencesTo(a)){
    Function f=getFunctionContaining(ref.getFromAddress());
    println(ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
    dc(f,d);
  }
  // Known node ctor/setup neighborhood.
  long[] xs={0x0046DA30L,0x0046DA80L,0x0046DB60L,0x0046DCA0L,0x0046DE60L};
  for(long x:xs)dc(getFunctionContaining(toAddr(x)),d);
  d.dispose();
 }
}