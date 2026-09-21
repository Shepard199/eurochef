import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.util.*;
public class AttackerTargetNavGlobalsEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  long[] globals={0x007B2A04L,0x007B2A0CL};
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  Set<Long> seen=new LinkedHashSet<>();
  for(long g:globals){
   println(String.format("\n=== refs 0x%08X ===",g));
   for(Reference ref:getReferencesTo(toAddr(g))){
    Function f=getFunctionContaining(ref.getFromAddress());
    println(ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
    if(f!=null)seen.add(f.getEntryPoint().getOffset());
   }
  }
  for(long a:seen){
   Function f=getFunctionAt(toAddr(a)); if(f==null)continue;
   println(String.format("\n=== FUNC 0x%08X %s ===",a,f.getName()));
   DecompileResults r=d.decompileFunction(f,120,monitor);
   if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  }
  d.dispose();
 }
}