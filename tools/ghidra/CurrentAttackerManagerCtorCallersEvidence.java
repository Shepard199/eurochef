import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.util.*;
public class CurrentAttackerManagerCtorCallersEvidence extends GhidraScript {
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  Set<Function> fs=new LinkedHashSet<>();
  for(Reference ref:getReferencesTo(toAddr(0x00456020L))){
    Function f=getFunctionContaining(ref.getFromAddress());
    println(ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
    if(f!=null)fs.add(f);
  }
  for(Function f:fs){
    println("\n=== "+f.getName()+"@"+f.getEntryPoint()+" ===");
    DecompileResults r=d.decompileFunction(f,150,monitor);
    if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  }
  d.dispose();
 }
}