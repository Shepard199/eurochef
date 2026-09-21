import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.util.*;
public class CurrentAttackerManagerCallersEvidence extends GhidraScript {
 private void dc(Function f,DecompInterface d)throws Exception{
  if(f==null)return; println("\n=== "+f.getName()+"@"+f.getEntryPoint()+" ===");
  DecompileResults r=d.decompileFunction(f,120,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  long[] targets={0x004561F0L,0x004563C0L,0x004552B0L,0x00455510L};
  Set<Function> fs=new LinkedHashSet<>();
  for(long t:targets){
   println(String.format("\n=== refs 0x%08X ===",t));
   for(Reference ref:getReferencesTo(toAddr(t))){
    Function f=getFunctionContaining(ref.getFromAddress());
    println(ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
    if(f!=null)fs.add(f);
   }
  }
  for(Function f:fs)dc(f,d);
  d.dispose();
 }
}