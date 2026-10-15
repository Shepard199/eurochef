import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.util.*;
public class InputManagerCtorChainEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(Function f)throws Exception{if(f==null)return;println("\n=== "+f.getName()+" @ "+f.getEntryPoint()+" ===");DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
 void level(long x,int depth,Set<Long> seen)throws Exception{
  if(depth>5||!seen.add(x))return;
  println(String.format("\n=== CALLERS OF 0x%08X depth=%d ===",x,depth));
  LinkedHashSet<Function> fs=new LinkedHashSet<>();
  ReferenceIterator it=currentProgram.getReferenceManager().getReferencesTo(toAddr(x));
  while(it.hasNext()){Reference r=it.next();if(!r.getReferenceType().isCall())continue;Function f=getFunctionContaining(r.getFromAddress());if(f!=null)fs.add(f);}
  for(Function f:fs){dec(f);level(f.getEntryPoint().getOffset(),depth+1,seen);}
 }
 public void run()throws Exception{d.openProgram(currentProgram);level(0x004f122aL,0,new HashSet<Long>());d.dispose();}
}