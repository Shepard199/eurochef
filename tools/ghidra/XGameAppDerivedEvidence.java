import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.symbol.*;
import ghidra.program.model.listing.Function;
public class XGameAppDerivedEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(Function f)throws Exception{
  if(f==null)return;
  println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
  d.openProgram(currentProgram);
  ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(toAddr(0x00431cb0L));
  while(it.hasNext()){
   Reference r=it.next();
   Function f=getFunctionContaining(r.getFromAddress());
   println(r.getReferenceType()+" "+r.getFromAddress()+" FROM "+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));
   if(f!=null)dec(f);
  }
  d.dispose();
 }
}