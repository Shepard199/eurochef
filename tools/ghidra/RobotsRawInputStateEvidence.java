import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.util.*;
public class RobotsRawInputStateEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(Function f)throws Exception{if(f==null)return;println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
 public void run()throws Exception{
  d.openProgram(currentProgram);
  LinkedHashSet<Function> fs=new LinkedHashSet<>();
  ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(toAddr(0x008cb0c8L));
  while(it.hasNext()){Reference r=it.next();Function f=getFunctionContaining(r.getFromAddress());println(r.getReferenceType()+" "+r.getFromAddress()+" "+(f==null?"<none>":f.getName()+"@"+f.getEntryPoint()));if(f!=null)fs.add(f);}
  int n=0;for(Function f:fs){if(n++<16)dec(f);}
  d.dispose();
 }
}