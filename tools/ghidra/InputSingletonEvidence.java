import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
public class InputSingletonEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(Function f)throws Exception{
  if(f==null)return;
  println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
  d.openProgram(currentProgram);
  ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(toAddr(0x007be16cL));
  int n=0;
  while(it.hasNext() && n<80){
    Reference r=it.next();
    Function f=getFunctionContaining(r.getFromAddress());
    println(r.getReferenceType()+" "+r.getFromAddress()+" FROM "+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));
    if(r.getReferenceType().isWrite() && f!=null) dec(f);
    n++;
  }
  d.dispose();
 }
}