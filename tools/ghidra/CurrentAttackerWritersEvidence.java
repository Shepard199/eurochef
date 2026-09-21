import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.util.LinkedHashSet;
import java.util.Set;
public class CurrentAttackerWritersEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address g=toAddr(0x007B2A10L);
  Set<Function> fs=new LinkedHashSet<>();
  for(Reference ref:getReferencesTo(g)){
   Function f=getFunctionContaining(ref.getFromAddress());
   println("REF "+ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
   if(f!=null) fs.add(f);
  }
  DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
  for(Function f:fs){
   println("\n=== "+f.getName()+" "+f.getEntryPoint()+" ===");
   DecompileResults r=d.decompileFunction(f,90,monitor);
   if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
  }
  d.dispose();
 }
}